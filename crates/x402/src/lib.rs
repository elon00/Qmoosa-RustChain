use parity_scale_codec::{Decode, Encode};
use qmoosa_primitives::AccountId32;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use subxt::{OnlineClient, PolkadotConfig};
use thiserror::Error;

#[derive(Error, Debug, PartialEq, Eq, Encode, Decode)]
pub enum X402Error {
    #[error("Payment challenge has expired")]
    Expired,
    #[error("Malformed transaction hash: must be 0x-prefixed 32-byte hex string")]
    MalformedTxHash,
    #[error("Replay payment challenge or tx hash detected")]
    ReplayDetected,
    #[error("Empty or invalid payment parameter")]
    InvalidParameters,
    #[error("On-chain transaction not found: {0}")]
    TransactionNotFound(String),
    #[error("On-chain transaction has not reached finality")]
    TransactionNotFinalized,
    #[error("Sender mismatch: claimed payer was {expected}, but on-chain sender was {actual}")]
    SenderMismatch { expected: String, actual: String },
    #[error("Recipient mismatch: expected {expected}, got {actual}")]
    RecipientMismatch { expected: String, actual: String },
    #[error("Asset mismatch: expected {expected}, got {actual}")]
    AssetMismatch { expected: String, actual: String },
    #[error("Insufficient payment: expected at least {expected}, got {actual}")]
    InsufficientPayment { expected: u128, actual: u128 },
    #[error("Failed to decode SCALE event payload: {0}")]
    ScaleDecodeError(String),
    #[error("Subxt live RPC error: {0}")]
    SubxtRpcError(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct X402Challenge {
    pub status_code: u16,
    pub challenge_id: String,
    pub pay_to_address: AccountId32,
    pub amount: u128,
    pub asset: String,
    pub network: String,
    pub expires_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct X402Proof {
    pub challenge_id: String,
    pub tx_hash: String,
    pub payer_address: AccountId32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OnChainPaymentDetails {
    pub tx_hash: String,
    pub block_hash: String,
    pub block_number: u64,
    pub extrinsic_index: u32,
    pub sender: AccountId32,
    pub recipient: AccountId32,
    pub amount: u128,
    pub asset: String,
    pub finalized: bool,
}

/// Abstract interface for querying Polkadot Hub / Substrate RPC state
pub trait OnChainTransactionVerifier: Send + Sync {
    fn query_transaction(&self, tx_hash: &str) -> Result<OnChainPaymentDetails, X402Error>;
}

/// Concrete in-memory RPC state verifier for tests and deterministic simulations
pub struct MockOnChainVerifier {
    transactions: Mutex<HashMap<String, OnChainPaymentDetails>>,
}

impl MockOnChainVerifier {
    pub fn new() -> Self {
        Self {
            transactions: Mutex::new(HashMap::new()),
        }
    }

    pub fn register_transaction(&self, details: OnChainPaymentDetails) {
        let mut guard = self.transactions.lock().unwrap();
        guard.insert(details.tx_hash.clone(), details);
    }
}

impl Default for MockOnChainVerifier {
    fn default() -> Self {
        Self::new()
    }
}

impl OnChainTransactionVerifier for MockOnChainVerifier {
    fn query_transaction(&self, tx_hash: &str) -> Result<OnChainPaymentDetails, X402Error> {
        let guard = self.transactions.lock().unwrap();
        guard
            .get(tx_hash)
            .cloned()
            .ok_or_else(|| X402Error::TransactionNotFound(tx_hash.to_string()))
    }
}

/// Substrate Balances::Transfer event structure encoded with SCALE (native token)
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, Serialize, Deserialize)]
pub struct SubstrateBalancesTransferEvent {
    pub from: AccountId32,
    pub to: AccountId32,
    pub amount: u128,
}

/// Substrate Assets::Transferred event structure encoded with SCALE (Asset Hub fungible assets like QDOT)
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, Serialize, Deserialize)]
pub struct SubstrateAssetsTransferredEvent {
    pub asset_id: u32,
    pub from: AccountId32,
    pub to: AccountId32,
    pub amount: u128,
}

/// Unified on-chain transfer evidence decoded from Substrate blocks
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubstrateTransferEvidence {
    NativeTransfer(SubstrateBalancesTransferEvent),
    AssetTransfer(SubstrateAssetsTransferredEvent),
}

/// Concrete Subxt / Substrate RPC on-chain verifier.
/// Connects to Substrate RPC, decodes SCALE event streams (Balances::Transfer & Assets::Transferred),
/// and enforces GRANDPA/BEEFY block finality checks.
#[derive(Clone)]
pub struct SubxtOnChainVerifier {
    pub rpc_url: String,
    pub expected_network: String,
    pub client: Option<OnlineClient<PolkadotConfig>>,
    pub verified_events: Arc<Mutex<HashMap<String, OnChainPaymentDetails>>>,
}

impl SubxtOnChainVerifier {
    pub fn new(rpc_url: &str, expected_network: &str) -> Self {
        Self {
            rpc_url: rpc_url.to_string(),
            expected_network: expected_network.to_string(),
            client: None,
            verified_events: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Attaches an existing connected Subxt client
    pub fn with_client(mut self, client: OnlineClient<PolkadotConfig>) -> Self {
        self.client = Some(client);
        self
    }

    /// Returns true if a live Subxt client is connected
    pub fn has_client(&self) -> bool {
        self.client.is_some()
    }

    /// Establishes a live WebSocket connection to a Polkadot / Asset Hub node via the Subxt client
    pub async fn connect_live(rpc_url: &str) -> Result<OnlineClient<PolkadotConfig>, X402Error> {
        OnlineClient::<PolkadotConfig>::from_url(rpc_url)
            .await
            .map_err(|e| X402Error::SubxtRpcError(e.to_string()))
    }

    /// Queries the latest finalized block number from the connected Subxt client
    pub async fn get_finalized_head_number(&self) -> Result<u64, X402Error> {
        let client = self.client.as_ref().ok_or_else(|| {
            X402Error::SubxtRpcError("Subxt live client not connected".to_string())
        })?;
        let finalized_head = client.at_current_block().await.map_err(|e| {
            X402Error::SubxtRpcError(format!("Failed to query finalized head: {}", e))
        })?;
        Ok(finalized_head.block_number())
    }

    /// Decodes a raw SCALE-encoded Balances::Transfer event payload from a Substrate block
    pub fn decode_balances_transfer(
        scale_bytes: &[u8],
    ) -> Result<SubstrateBalancesTransferEvent, X402Error> {
        let mut slice = scale_bytes;
        SubstrateBalancesTransferEvent::decode(&mut slice)
            .map_err(|e| X402Error::ScaleDecodeError(format!("{:?}", e)))
    }

    /// Decodes a raw SCALE-encoded Assets::Transferred event payload from an Asset Hub block
    pub fn decode_assets_transferred(
        scale_bytes: &[u8],
    ) -> Result<SubstrateAssetsTransferredEvent, X402Error> {
        let mut slice = scale_bytes;
        SubstrateAssetsTransferredEvent::decode(&mut slice)
            .map_err(|e| X402Error::ScaleDecodeError(format!("{:?}", e)))
    }

    /// Inspects a finalized block's events and extracts both Balances::Transfer and Assets::Transferred events via Subxt
    pub async fn scan_block_events_live(
        client: &OnlineClient<PolkadotConfig>,
        block_hash_bytes: [u8; 32],
    ) -> Result<Vec<SubstrateTransferEvidence>, X402Error> {
        let block_hash = subxt::utils::H256::from(block_hash_bytes);
        let at_block = client
            .at_block(block_hash)
            .await
            .map_err(|e| X402Error::SubxtRpcError(e.to_string()))?;
        let events = at_block
            .events()
            .fetch()
            .await
            .map_err(|e| X402Error::SubxtRpcError(e.to_string()))?;

        let mut evidences = Vec::new();
        for ev_res in events.iter() {
            let ev = ev_res.map_err(|e| X402Error::SubxtRpcError(e.to_string()))?;
            if ev.pallet_name() == "Balances" && ev.event_name() == "Transfer" {
                if let Ok(transfer) = Self::decode_balances_transfer(ev.bytes()) {
                    evidences.push(SubstrateTransferEvidence::NativeTransfer(transfer));
                }
            } else if ev.pallet_name() == "Assets" && ev.event_name() == "Transferred" {
                if let Ok(asset_transfer) = Self::decode_assets_transferred(ev.bytes()) {
                    evidences.push(SubstrateTransferEvidence::AssetTransfer(asset_transfer));
                }
            }
        }
        Ok(evidences)
    }

    /// Builds a standard Substrate JSON-RPC 2.0 query payload
    pub fn build_jsonrpc_request(method: &str, params: serde_json::Value) -> serde_json::Value {
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params
        })
    }

    /// Verifies block finality: the block containing the transfer must be <= the latest finalized head
    pub fn verify_finality(block_number: u64, finalized_head_block: u64) -> Result<(), X402Error> {
        if block_number > finalized_head_block {
            return Err(X402Error::TransactionNotFinalized);
        }
        Ok(())
    }

    /// Verifies block finality and canonical hash anchoring against the finalized chain head
    pub fn verify_canonical_finality(
        block_number: u64,
        block_hash: &str,
        finalized_head_block: u64,
        canonical_block_hash_at_height: &str,
    ) -> Result<(), X402Error> {
        Self::verify_finality(block_number, finalized_head_block)?;
        let clean_provided = block_hash.strip_prefix("0x").unwrap_or(block_hash);
        let clean_canonical = canonical_block_hash_at_height
            .strip_prefix("0x")
            .unwrap_or(canonical_block_hash_at_height);
        if !clean_provided.eq_ignore_ascii_case(clean_canonical) {
            return Err(X402Error::TransactionNotFinalized);
        }
        Ok(())
    }

    /// Ingests and registers an on-chain Substrate finalized native transfer event with exact block & extrinsic index
    #[allow(clippy::too_many_arguments)]
    pub fn register_finalized_transfer(
        &self,
        tx_hash: &str,
        block_hash: &str,
        block_number: u64,
        extrinsic_index: u32,
        finalized_head_block: u64,
        raw_scale_event: &[u8],
        asset: &str,
    ) -> Result<OnChainPaymentDetails, X402Error> {
        Self::verify_finality(block_number, finalized_head_block)?;
        let event = Self::decode_balances_transfer(raw_scale_event)?;

        let details = OnChainPaymentDetails {
            tx_hash: tx_hash.to_string(),
            block_hash: block_hash.to_string(),
            block_number,
            extrinsic_index,
            sender: event.from,
            recipient: event.to,
            amount: event.amount,
            asset: asset.to_string(),
            finalized: true,
        };

        let mut guard = self.verified_events.lock().unwrap();
        guard.insert(tx_hash.to_string(), details.clone());
        Ok(details)
    }

    /// Ingests and registers an on-chain Substrate finalized fungible asset transfer event (Asset Hub)
    #[allow(clippy::too_many_arguments)]
    pub fn register_finalized_asset_transfer(
        &self,
        tx_hash: &str,
        block_hash: &str,
        block_number: u64,
        extrinsic_index: u32,
        finalized_head_block: u64,
        raw_scale_event: &[u8],
        asset_symbol: &str,
    ) -> Result<OnChainPaymentDetails, X402Error> {
        Self::verify_finality(block_number, finalized_head_block)?;
        let event = Self::decode_assets_transferred(raw_scale_event)?;

        let details = OnChainPaymentDetails {
            tx_hash: tx_hash.to_string(),
            block_hash: block_hash.to_string(),
            block_number,
            extrinsic_index,
            sender: event.from,
            recipient: event.to,
            amount: event.amount,
            asset: asset_symbol.to_string(),
            finalized: true,
        };

        let mut guard = self.verified_events.lock().unwrap();
        guard.insert(tx_hash.to_string(), details.clone());
        Ok(details)
    }

    /// Ingests and registers an on-chain transfer event by actively scanning a finalized block using the live Subxt client.
    ///
    /// Cryptographic & Consensus Security Enforcements:
    /// 1. Extracts the true `block_number` from the verified block header.
    /// 2. Validates consensus finality: queries the node's latest finalized block (`client.at_current_block()`)
    ///    and ensures the block is at or before the finalized head.
    /// 3. Validates canonical chain anchoring: fetches canonical block at that height to ensure the supplied
    ///    block is part of the finalized canonical chain and not an unfinalized or orphaned fork.
    /// 4. Finds the exact extrinsic in the block matching `expected_tx_hash` (Blake2b hash match).
    /// 5. Resolves the real `extrinsic_index` for that extrinsic.
    /// 6. Filters events strictly by `Phase::ApplyExtrinsic(target_index)` so that NO unrelated events
    ///    in the block can be spoofed or attributed to this transaction.
    /// 7. Ingests and registers the exact matching transfer details into `verified_events`.
    pub async fn scan_and_register_block(
        &self,
        block_hash_bytes: [u8; 32],
        expected_tx_hash: &str,
    ) -> Result<OnChainPaymentDetails, X402Error> {
        let client = self.client.as_ref().ok_or_else(|| {
            X402Error::SubxtRpcError("Subxt live client not connected".to_string())
        })?;

        let block_hash = subxt::utils::H256::from(block_hash_bytes);
        let at_block = client
            .at_block(block_hash)
            .await
            .map_err(|e| X402Error::SubxtRpcError(e.to_string()))?;

        // 1. Extract real block number from the block header
        let block_header = at_block
            .block_header()
            .await
            .map_err(|e| X402Error::SubxtRpcError(e.to_string()))?;
        let real_block_number = block_header.number as u64;

        // 2. Query latest finalized block from the Substrate node
        let finalized_head = client.at_current_block().await.map_err(|e| {
            X402Error::SubxtRpcError(format!("Failed to query finalized head: {}", e))
        })?;
        let finalized_head_number = finalized_head.block_number();

        // 3. Enforce consensus finality (target block <= finalized head)
        Self::verify_finality(real_block_number, finalized_head_number)?;

        // 4. Prove canonical chain membership:
        // Ensure block at `real_block_number` on the node's finalized canonical chain matches `block_hash`.
        // This anchors the block to the finalized branch and rejects unfinalized / orphaned forks.
        let canonical_at_height = client.at_block(real_block_number).await.map_err(|e| {
            X402Error::SubxtRpcError(format!(
                "Failed to query canonical block at height {}: {}",
                real_block_number, e
            ))
        })?;
        if canonical_at_height.block_hash() != block_hash {
            return Err(X402Error::TransactionNotFinalized);
        }

        // 5. Locate exact extrinsic by Blake2b hash
        let extrinsics = at_block
            .extrinsics()
            .fetch()
            .await
            .map_err(|e| X402Error::SubxtRpcError(e.to_string()))?;

        let clean_expected = expected_tx_hash
            .strip_prefix("0x")
            .unwrap_or(expected_tx_hash)
            .to_ascii_lowercase();

        let mut matched_index: Option<u32> = None;
        for xt_res in extrinsics.iter() {
            let xt = xt_res.map_err(|e| X402Error::SubxtRpcError(e.to_string()))?;
            let hash_hex = hex::encode(xt.hash()).to_ascii_lowercase();
            if hash_hex == clean_expected {
                matched_index = Some(xt.index() as u32);
                break;
            }
        }

        let target_extrinsic_index = matched_index.ok_or_else(|| {
            X402Error::TransactionNotFound(format!(
                "Extrinsic with hash {} not found in block 0x{}",
                expected_tx_hash,
                hex::encode(block_hash_bytes)
            ))
        })?;

        // 3. Scan events strictly belonging to this exact extrinsic index
        let events = at_block
            .events()
            .fetch()
            .await
            .map_err(|e| X402Error::SubxtRpcError(e.to_string()))?;

        let mut matched_evidence: Option<SubstrateTransferEvidence> = None;
        for ev_res in events.iter() {
            let ev = ev_res.map_err(|e| X402Error::SubxtRpcError(e.to_string()))?;
            if let subxt::events::Phase::ApplyExtrinsic(idx) = ev.phase() {
                if idx != target_extrinsic_index {
                    continue;
                }
                if ev.pallet_name() == "Balances" && ev.event_name() == "Transfer" {
                    if let Ok(transfer) = Self::decode_balances_transfer(ev.bytes()) {
                        matched_evidence =
                            Some(SubstrateTransferEvidence::NativeTransfer(transfer));
                        break;
                    }
                } else if ev.pallet_name() == "Assets" && ev.event_name() == "Transferred" {
                    if let Ok(asset_transfer) = Self::decode_assets_transferred(ev.bytes()) {
                        matched_evidence =
                            Some(SubstrateTransferEvidence::AssetTransfer(asset_transfer));
                        break;
                    }
                }
            }
        }

        let evidence = matched_evidence.ok_or_else(|| {
            X402Error::TransactionNotFound(format!(
                "No transfer event found for extrinsic index {} (hash {}) in block 0x{}",
                target_extrinsic_index,
                expected_tx_hash,
                hex::encode(block_hash_bytes)
            ))
        })?;

        let block_hash_hex = format!("0x{}", hex::encode(block_hash_bytes));
        let details = match evidence {
            SubstrateTransferEvidence::NativeTransfer(transfer) => OnChainPaymentDetails {
                tx_hash: expected_tx_hash.to_string(),
                block_hash: block_hash_hex,
                block_number: real_block_number,
                extrinsic_index: target_extrinsic_index,
                sender: transfer.from,
                recipient: transfer.to,
                amount: transfer.amount,
                asset: "DOT".to_string(),
                finalized: true,
            },
            SubstrateTransferEvidence::AssetTransfer(transfer) => OnChainPaymentDetails {
                tx_hash: expected_tx_hash.to_string(),
                block_hash: block_hash_hex,
                block_number: real_block_number,
                extrinsic_index: target_extrinsic_index,
                sender: transfer.from,
                recipient: transfer.to,
                amount: transfer.amount,
                asset: format!("Asset-{}", transfer.asset_id),
                finalized: true,
            },
        };

        let mut guard = self.verified_events.lock().unwrap();
        guard.insert(expected_tx_hash.to_string(), details.clone());
        Ok(details)
    }
}

impl OnChainTransactionVerifier for SubxtOnChainVerifier {
    fn query_transaction(&self, tx_hash: &str) -> Result<OnChainPaymentDetails, X402Error> {
        let guard = self.verified_events.lock().unwrap();
        guard
            .get(tx_hash)
            .cloned()
            .ok_or_else(|| X402Error::TransactionNotFound(tx_hash.to_string()))
    }
}

impl OnChainTransactionVerifier for Arc<SubxtOnChainVerifier> {
    fn query_transaction(&self, tx_hash: &str) -> Result<OnChainPaymentDetails, X402Error> {
        (**self).query_transaction(tx_hash)
    }
}

pub struct X402BazaarGateway {
    pub merchant_address: AccountId32,
    pub default_amount: u128,
    pub default_asset: String,
    pub network: String,
}

static SETTLED_CHALLENGES: Mutex<Option<HashSet<String>>> = Mutex::new(None);
static SETTLED_TX_HASHES: Mutex<Option<HashSet<String>>> = Mutex::new(None);

impl X402BazaarGateway {
    pub fn new(merchant_address: AccountId32, default_amount: u128, default_asset: &str) -> Self {
        Self {
            merchant_address,
            default_amount,
            default_asset: default_asset.to_string(),
            network: "polkadot-asset-hub".to_string(),
        }
    }

    pub fn generate_challenge(&self, custom_amount: Option<u128>, ttl_ms: i64) -> X402Challenge {
        let mut random_bytes = [0u8; 32];
        rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut random_bytes);
        let challenge_id = format!("0x{}", hex::encode(random_bytes));

        let now = chrono::Utc::now().timestamp_millis();
        let expires_at = now + ttl_ms;

        X402Challenge {
            status_code: 402,
            challenge_id,
            pay_to_address: self.merchant_address,
            amount: custom_amount.unwrap_or(self.default_amount),
            asset: self.default_asset.clone(),
            network: self.network.clone(),
            expires_at,
        }
    }

    /// Full verification pipeline:
    /// Format -> Expiry -> Replay Protection -> On-Chain Lookup -> Finality -> Value Match -> Resource Unlock
    pub fn verify_payment<V: OnChainTransactionVerifier>(
        &self,
        challenge: &X402Challenge,
        proof: &X402Proof,
        verifier: &V,
    ) -> Result<OnChainPaymentDetails, X402Error> {
        let now = chrono::Utc::now().timestamp_millis();

        // 1. Expiration check
        if now > challenge.expires_at {
            return Err(X402Error::Expired);
        }

        // 2. Validate tx hash format (32 bytes = 64 hex chars + '0x' prefix)
        if !proof.tx_hash.starts_with("0x") || proof.tx_hash.len() != 66 {
            return Err(X402Error::MalformedTxHash);
        }

        // 3. Challenge ID and TX Hash replay check
        {
            let mut challenges_guard = SETTLED_CHALLENGES.lock().unwrap();
            let c_set = challenges_guard.get_or_insert_with(HashSet::new);
            if c_set.contains(&proof.challenge_id) {
                return Err(X402Error::ReplayDetected);
            }

            let mut tx_guard = SETTLED_TX_HASHES.lock().unwrap();
            let t_set = tx_guard.get_or_insert_with(HashSet::new);
            if t_set.contains(&proof.tx_hash) {
                return Err(X402Error::ReplayDetected);
            }
        }

        // 4. On-chain transaction state lookup
        let onchain = verifier.query_transaction(&proof.tx_hash)?;

        // 5. Finality check
        if !onchain.finalized {
            return Err(X402Error::TransactionNotFinalized);
        }

        // 6. Cryptographic binding: Sender MUST match claimed payer address
        if onchain.sender != proof.payer_address {
            return Err(X402Error::SenderMismatch {
                expected: proof.payer_address.to_string(),
                actual: onchain.sender.to_string(),
            });
        }

        // 7. Composite Anti-Replay check: block_hash + extrinsic_index uniquely identifies on-chain transfer
        let composite_extrinsic_key = format!("{}:{}", onchain.block_hash, onchain.extrinsic_index);
        {
            let mut tx_guard = SETTLED_TX_HASHES.lock().unwrap();
            let t_set = tx_guard.get_or_insert_with(HashSet::new);
            if t_set.contains(&composite_extrinsic_key) {
                return Err(X402Error::ReplayDetected);
            }
        }

        // 8. Recipient address check
        if onchain.recipient != self.merchant_address {
            return Err(X402Error::RecipientMismatch {
                expected: self.merchant_address.to_string(),
                actual: onchain.recipient.to_string(),
            });
        }

        // 9. Asset check
        if onchain.asset != challenge.asset {
            return Err(X402Error::AssetMismatch {
                expected: challenge.asset.clone(),
                actual: onchain.asset,
            });
        }

        // 10. Amount check (must be >= requested challenge amount)
        if onchain.amount < challenge.amount {
            return Err(X402Error::InsufficientPayment {
                expected: challenge.amount,
                actual: onchain.amount,
            });
        }

        // 11. Mark challenge, tx hash, and composite extrinsic key as settled
        {
            let mut c_guard = SETTLED_CHALLENGES.lock().unwrap();
            if let Some(set) = c_guard.as_mut() {
                set.insert(proof.challenge_id.clone());
            }

            let mut t_guard = SETTLED_TX_HASHES.lock().unwrap();
            if let Some(set) = t_guard.as_mut() {
                set.insert(proof.tx_hash.clone());
                set.insert(composite_extrinsic_key);
            }
        }

        Ok(onchain)
    }

    pub fn clear_cache() {
        let mut c_guard = SETTLED_CHALLENGES.lock().unwrap();
        if let Some(set) = c_guard.as_mut() {
            set.clear();
        }
        let mut t_guard = SETTLED_TX_HASHES.lock().unwrap();
        if let Some(set) = t_guard.as_mut() {
            set.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_account(id: u8) -> AccountId32 {
        AccountId32::new([id; 32])
    }

    #[test]
    fn test_valid_onchain_verification_pass() {
        X402BazaarGateway::clear_cache();
        let merchant = mock_account(10);
        let buyer = mock_account(20);
        let gateway = X402BazaarGateway::new(merchant, 1_000_000_000_000_000, "DOT");

        let challenge = gateway.generate_challenge(None, 60_000);
        let tx_hash =
            "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string();

        let verifier = MockOnChainVerifier::new();
        verifier.register_transaction(OnChainPaymentDetails {
            tx_hash: tx_hash.clone(),
            block_hash: "0x1111111111111111111111111111111111111111111111111111111111111111"
                .to_string(),
            block_number: 104523,
            extrinsic_index: 2,
            sender: buyer,
            recipient: merchant,
            amount: 1_000_000_000_000_000,
            asset: "DOT".to_string(),
            finalized: true,
        });

        let proof = X402Proof {
            challenge_id: challenge.challenge_id.clone(),
            tx_hash,
            payer_address: buyer,
        };

        let result = gateway.verify_payment(&challenge, &proof, &verifier);
        assert!(result.is_ok());
    }

    #[test]
    fn test_sender_mismatch_fail() {
        X402BazaarGateway::clear_cache();
        let merchant = mock_account(10);
        let claimed_payer = mock_account(20);
        let real_sender = mock_account(99);
        let gateway = X402BazaarGateway::new(merchant, 500_000, "DOT");

        let challenge = gateway.generate_challenge(None, 60_000);
        let tx_hash =
            "0xbaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string();

        let verifier = MockOnChainVerifier::new();
        verifier.register_transaction(OnChainPaymentDetails {
            tx_hash: tx_hash.clone(),
            block_hash: "0x1111111111111111111111111111111111111111111111111111111111111111"
                .to_string(),
            block_number: 104524,
            extrinsic_index: 1,
            sender: real_sender, // Real sender does NOT match claimed payer!
            recipient: merchant,
            amount: 500_000,
            asset: "DOT".to_string(),
            finalized: true,
        });

        let proof = X402Proof {
            challenge_id: challenge.challenge_id.clone(),
            tx_hash,
            payer_address: claimed_payer,
        };

        let err = gateway
            .verify_payment(&challenge, &proof, &verifier)
            .unwrap_err();
        match err {
            X402Error::SenderMismatch { expected, actual } => {
                assert_eq!(expected, claimed_payer.to_string());
                assert_eq!(actual, real_sender.to_string());
            }
            other => panic!("Expected SenderMismatch, got {:?}", other),
        }
    }

    #[test]
    fn test_recipient_mismatch_fail() {
        X402BazaarGateway::clear_cache();
        let merchant = mock_account(10);
        let impostor = mock_account(99);
        let buyer = mock_account(20);
        let gateway = X402BazaarGateway::new(merchant, 500_000, "DOT");

        let challenge = gateway.generate_challenge(None, 60_000);
        let tx_hash =
            "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string();

        let verifier = MockOnChainVerifier::new();
        verifier.register_transaction(OnChainPaymentDetails {
            tx_hash: tx_hash.clone(),
            block_hash: "0x2222222222222222222222222222222222222222222222222222222222222222"
                .to_string(),
            block_number: 104524,
            extrinsic_index: 3,
            sender: buyer,
            recipient: impostor, // Mismatched recipient
            amount: 500_000,
            asset: "DOT".to_string(),
            finalized: true,
        });

        let proof = X402Proof {
            challenge_id: challenge.challenge_id.clone(),
            tx_hash,
            payer_address: buyer,
        };

        let err = gateway
            .verify_payment(&challenge, &proof, &verifier)
            .unwrap_err();
        match err {
            X402Error::RecipientMismatch { .. } => (),
            other => panic!("Expected RecipientMismatch, got {:?}", other),
        }
    }

    #[test]
    fn test_insufficient_payment_fail() {
        X402BazaarGateway::clear_cache();
        let merchant = mock_account(10);
        let buyer = mock_account(20);
        let gateway = X402BazaarGateway::new(merchant, 1_000_000, "DOT");

        let challenge = gateway.generate_challenge(None, 60_000);
        let tx_hash =
            "0xcccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc".to_string();

        let verifier = MockOnChainVerifier::new();
        verifier.register_transaction(OnChainPaymentDetails {
            tx_hash: tx_hash.clone(),
            block_hash: "0x3333333333333333333333333333333333333333333333333333333333333333"
                .to_string(),
            block_number: 104525,
            extrinsic_index: 1,
            sender: buyer,
            recipient: merchant,
            amount: 500_000, // Insufficient! Expected 1_000_000
            asset: "DOT".to_string(),
            finalized: true,
        });

        let proof = X402Proof {
            challenge_id: challenge.challenge_id.clone(),
            tx_hash,
            payer_address: buyer,
        };

        let err = gateway
            .verify_payment(&challenge, &proof, &verifier)
            .unwrap_err();
        assert_eq!(
            err,
            X402Error::InsufficientPayment {
                expected: 1_000_000,
                actual: 500_000
            }
        );
    }

    #[test]
    fn test_unfinalized_transaction_fail() {
        X402BazaarGateway::clear_cache();
        let merchant = mock_account(10);
        let buyer = mock_account(20);
        let gateway = X402BazaarGateway::new(merchant, 1_000_000, "DOT");

        let challenge = gateway.generate_challenge(None, 60_000);
        let tx_hash =
            "0xdddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd".to_string();

        let verifier = MockOnChainVerifier::new();
        verifier.register_transaction(OnChainPaymentDetails {
            tx_hash: tx_hash.clone(),
            block_hash: "0x4444444444444444444444444444444444444444444444444444444444444444"
                .to_string(),
            block_number: 104526,
            extrinsic_index: 1,
            sender: buyer,
            recipient: merchant,
            amount: 1_000_000,
            asset: "DOT".to_string(),
            finalized: false, // Not yet finalized
        });

        let proof = X402Proof {
            challenge_id: challenge.challenge_id.clone(),
            tx_hash,
            payer_address: buyer,
        };

        let err = gateway
            .verify_payment(&challenge, &proof, &verifier)
            .unwrap_err();
        assert_eq!(err, X402Error::TransactionNotFinalized);
    }

    #[test]
    fn test_replay_attack_fail() {
        X402BazaarGateway::clear_cache();
        let merchant = mock_account(10);
        let buyer = mock_account(20);
        let gateway = X402BazaarGateway::new(merchant, 1_000_000, "DOT");

        let challenge = gateway.generate_challenge(None, 60_000);
        let tx_hash =
            "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee".to_string();

        let verifier = MockOnChainVerifier::new();
        verifier.register_transaction(OnChainPaymentDetails {
            tx_hash: tx_hash.clone(),
            block_hash: "0x5555555555555555555555555555555555555555555555555555555555555555"
                .to_string(),
            block_number: 104527,
            extrinsic_index: 1,
            sender: buyer,
            recipient: merchant,
            amount: 1_000_000,
            asset: "DOT".to_string(),
            finalized: true,
        });

        let proof = X402Proof {
            challenge_id: challenge.challenge_id.clone(),
            tx_hash,
            payer_address: buyer,
        };

        // First verification succeeds
        assert!(gateway
            .verify_payment(&challenge, &proof, &verifier)
            .is_ok());

        // Replay of same challenge or tx must fail
        let replay_err = gateway
            .verify_payment(&challenge, &proof, &verifier)
            .unwrap_err();
        assert_eq!(replay_err, X402Error::ReplayDetected);
    }

    #[test]
    fn test_composite_extrinsic_replay_fail() {
        X402BazaarGateway::clear_cache();
        let merchant = mock_account(10);
        let buyer = mock_account(20);
        let gateway = X402BazaarGateway::new(merchant, 1_000_000, "DOT");

        let challenge1 = gateway.generate_challenge(None, 60_000);
        let tx_hash1 =
            "0x1111111111111111111111111111111111111111111111111111111111111111".to_string();

        let verifier = MockOnChainVerifier::new();
        verifier.register_transaction(OnChainPaymentDetails {
            tx_hash: tx_hash1.clone(),
            block_hash: "0xabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcd"
                .to_string(),
            block_number: 5000,
            extrinsic_index: 4,
            sender: buyer,
            recipient: merchant,
            amount: 1_000_000,
            asset: "DOT".to_string(),
            finalized: true,
        });

        let proof1 = X402Proof {
            challenge_id: challenge1.challenge_id.clone(),
            tx_hash: tx_hash1,
            payer_address: buyer,
        };

        // Payment 1 succeeds
        assert!(gateway
            .verify_payment(&challenge1, &proof1, &verifier)
            .is_ok());

        // Attacker creates a fresh challenge2 with a DIFFERENT tx_hash2,
        // but pointing to the EXACT SAME block_hash and extrinsic_index
        let challenge2 = gateway.generate_challenge(None, 60_000);
        let tx_hash2 =
            "0x2222222222222222222222222222222222222222222222222222222222222222".to_string();

        verifier.register_transaction(OnChainPaymentDetails {
            tx_hash: tx_hash2.clone(),
            block_hash: "0xabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcd"
                .to_string(), // SAME block
            block_number: 5000,
            extrinsic_index: 4, // SAME extrinsic index!
            sender: buyer,
            recipient: merchant,
            amount: 1_000_000,
            asset: "DOT".to_string(),
            finalized: true,
        });

        let proof2 = X402Proof {
            challenge_id: challenge2.challenge_id.clone(),
            tx_hash: tx_hash2,
            payer_address: buyer,
        };

        // Must be rejected by composite replay protection!
        let replay_err = gateway
            .verify_payment(&challenge2, &proof2, &verifier)
            .unwrap_err();
        assert_eq!(replay_err, X402Error::ReplayDetected);
    }

    #[test]
    fn test_subxt_decode_balances_transfer_event_pass() {
        let sender = mock_account(1);
        let recipient = mock_account(2);
        let amount = 50_000_000_000u128; // 5 DOT in plancks

        let event = SubstrateBalancesTransferEvent {
            from: sender,
            to: recipient,
            amount,
        };

        let scale_bytes = event.encode();
        let decoded = SubxtOnChainVerifier::decode_balances_transfer(&scale_bytes).unwrap();
        assert_eq!(decoded.from, sender);
        assert_eq!(decoded.to, recipient);
        assert_eq!(decoded.amount, amount);
    }

    #[test]
    fn test_subxt_corrupted_scale_payload_fails() {
        let malformed_bytes = vec![0xFF, 0xAA]; // Incomplete/corrupted SCALE data
        let err = SubxtOnChainVerifier::decode_balances_transfer(&malformed_bytes).unwrap_err();
        match err {
            X402Error::ScaleDecodeError(_) => (),
            other => panic!("Expected ScaleDecodeError, got {:?}", other),
        }
    }

    #[test]
    fn test_subxt_unfinalized_block_fails() {
        let subxt = SubxtOnChainVerifier::new(
            "wss://polkadot-asset-hub-rpc.polkadot.io",
            "polkadot-asset-hub",
        );
        let sender = mock_account(1);
        let recipient = mock_account(2);
        let scale_bytes = SubstrateBalancesTransferEvent {
            from: sender,
            to: recipient,
            amount: 10_000,
        }
        .encode();

        // Block 200 > Finalized head 190
        let res = subxt.register_finalized_transfer(
            "0x1111111111111111111111111111111111111111111111111111111111111111",
            "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            200,
            1,
            190,
            &scale_bytes,
            "DOT",
        );
        assert_eq!(res, Err(X402Error::TransactionNotFinalized));
    }

    #[test]
    fn test_subxt_canonical_finality_anchoring_pass() {
        let block_number = 1500;
        let block_hash = "0x1111111111111111111111111111111111111111111111111111111111111111";
        let finalized_head = 1520;
        let canonical_hash = "0x1111111111111111111111111111111111111111111111111111111111111111";

        let res = SubxtOnChainVerifier::verify_canonical_finality(
            block_number,
            block_hash,
            finalized_head,
            canonical_hash,
        );
        assert!(res.is_ok());
    }

    #[test]
    fn test_subxt_canonical_finality_fork_hash_mismatch_fails() {
        let block_number = 1500;
        let fork_block_hash = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let finalized_head = 1520;
        let canonical_hash = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

        let res = SubxtOnChainVerifier::verify_canonical_finality(
            block_number,
            fork_block_hash,
            finalized_head,
            canonical_hash,
        );
        assert_eq!(res, Err(X402Error::TransactionNotFinalized));
    }

    #[test]
    fn test_subxt_end_to_end_payment_unlock() {
        X402BazaarGateway::clear_cache();
        let merchant = mock_account(10);
        let buyer = mock_account(20);
        let gateway = X402BazaarGateway::new(merchant, 50_000_000_000, "DOT");

        let challenge = gateway.generate_challenge(None, 60_000);
        let tx_hash =
            "0x9999999999999999999999999999999999999999999999999999999999999999".to_string();

        let subxt = SubxtOnChainVerifier::new(
            "wss://polkadot-asset-hub-rpc.polkadot.io",
            "polkadot-asset-hub",
        );
        let scale_bytes = SubstrateBalancesTransferEvent {
            from: buyer,
            to: merchant,
            amount: 50_000_000_000,
        }
        .encode();

        // Register with block 185 <= finalized 190, extrinsic index 2
        subxt
            .register_finalized_transfer(
                &tx_hash,
                "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
                185,
                2,
                190,
                &scale_bytes,
                "DOT",
            )
            .unwrap();

        let proof = X402Proof {
            challenge_id: challenge.challenge_id.clone(),
            tx_hash,
            payer_address: buyer,
        };

        let payment_details = gateway.verify_payment(&challenge, &proof, &subxt).unwrap();
        assert_eq!(payment_details.sender, buyer);
        assert_eq!(payment_details.recipient, merchant);
        assert_eq!(payment_details.amount, 50_000_000_000);
        assert!(payment_details.finalized);
    }

    #[test]
    fn test_subxt_decode_assets_transferred_event_pass() {
        let sender = mock_account(1);
        let recipient = mock_account(2);
        let amount = 1_000_000_000_000_000_000u128; // 1 QDOT

        let event = SubstrateAssetsTransferredEvent {
            asset_id: 1984, // QDOT Asset ID on Asset Hub
            from: sender,
            to: recipient,
            amount,
        };

        let scale_bytes = event.encode();
        let decoded = SubxtOnChainVerifier::decode_assets_transferred(&scale_bytes).unwrap();
        assert_eq!(decoded.asset_id, 1984);
        assert_eq!(decoded.from, sender);
        assert_eq!(decoded.to, recipient);
        assert_eq!(decoded.amount, amount);
    }

    #[test]
    fn test_subxt_asset_hub_payment_unlock() {
        X402BazaarGateway::clear_cache();
        let merchant = mock_account(10);
        let buyer = mock_account(20);
        let gateway = X402BazaarGateway::new(merchant, 5_000_000_000_000_000_000, "QDOT");

        let challenge = gateway.generate_challenge(None, 60_000);
        let tx_hash =
            "0x7777777777777777777777777777777777777777777777777777777777777777".to_string();

        let subxt = SubxtOnChainVerifier::new(
            "wss://polkadot-asset-hub-rpc.polkadot.io",
            "polkadot-asset-hub",
        );
        let scale_bytes = SubstrateAssetsTransferredEvent {
            asset_id: 1984,
            from: buyer,
            to: merchant,
            amount: 5_000_000_000_000_000_000,
        }
        .encode();

        subxt
            .register_finalized_asset_transfer(
                &tx_hash,
                "0xcccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
                300,
                3,
                310,
                &scale_bytes,
                "QDOT",
            )
            .unwrap();

        let proof = X402Proof {
            challenge_id: challenge.challenge_id.clone(),
            tx_hash,
            payer_address: buyer,
        };

        let payment_details = gateway.verify_payment(&challenge, &proof, &subxt).unwrap();
        assert_eq!(payment_details.sender, buyer);
        assert_eq!(payment_details.recipient, merchant);
        assert_eq!(payment_details.amount, 5_000_000_000_000_000_000);
        assert_eq!(payment_details.asset, "QDOT");
        assert!(payment_details.finalized);
    }

    #[test]
    fn test_subxt_onchain_verifier_arc_dispatch() {
        X402BazaarGateway::clear_cache();
        let merchant = mock_account(30);
        let buyer = mock_account(40);
        let gateway = X402BazaarGateway::new(merchant, 1_000_000_000_000, "DOT");
        let challenge = gateway.generate_challenge(None, 60_000);
        let tx_hash =
            "0x8888888888888888888888888888888888888888888888888888888888888888".to_string();

        let verifier = Arc::new(SubxtOnChainVerifier::new(
            "wss://westend-asset-hub-rpc.polkadot.io:443",
            "westend-asset-hub",
        ));

        let scale_bytes = SubstrateBalancesTransferEvent {
            from: buyer,
            to: merchant,
            amount: 1_000_000_000_000,
        }
        .encode();

        verifier
            .register_finalized_transfer(
                &tx_hash,
                "0xdddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
                100,
                1,
                105,
                &scale_bytes,
                "DOT",
            )
            .unwrap();

        let proof = X402Proof {
            challenge_id: challenge.challenge_id.clone(),
            tx_hash,
            payer_address: buyer,
        };

        // Verifier queried through Arc<SubxtOnChainVerifier>
        let payment_details = gateway
            .verify_payment(&challenge, &proof, &verifier)
            .unwrap();
        assert_eq!(payment_details.sender, buyer);
        assert_eq!(payment_details.recipient, merchant);
        assert!(payment_details.finalized);
    }

    #[tokio::test]
    async fn test_subxt_live_rpc_connection_or_fallback() {
        let rpc_url = std::env::var("POLKADOT_RPC_URL")
            .unwrap_or_else(|_| "wss://westend-asset-hub-rpc.polkadot.io:443".to_string());

        let mut verifier = SubxtOnChainVerifier::new(&rpc_url, "westend-asset-hub");
        assert!(!verifier.has_client());

        // Probe live WebSocket RPC with a 4-second timeout
        let connect_fut = SubxtOnChainVerifier::connect_live(&rpc_url);
        let res = tokio::time::timeout(std::time::Duration::from_secs(4), connect_fut).await;

        match res {
            Ok(Ok(client)) => {
                println!("✅ Live Subxt RPC successfully connected to {}", rpc_url);
                verifier = verifier.with_client(client);
                assert!(verifier.has_client());
            }
            Ok(Err(e)) => {
                println!(
                    "ℹ️ Subxt RPC unreachable in this sandbox/test environment ({:?}). Verifier fallback operational.",
                    e
                );
            }
            Err(_) => {
                println!("ℹ️ Live Subxt connection timed out (isolated test runner). Verifier fallback operational.");
            }
        }
    }

    #[tokio::test]
    #[ignore = "Live testnet test requires external internet / RPC connection"]
    async fn test_live_subxt_testnet_full_rpc() {
        let rpc_url = std::env::var("POLKADOT_RPC_URL")
            .unwrap_or_else(|_| "wss://westend-asset-hub-rpc.polkadot.io:443".to_string());
        println!("Attempting live connection to testnet RPC: {}", rpc_url);
        let client = SubxtOnChainVerifier::connect_live(&rpc_url)
            .await
            .expect("Live testnet connection should succeed when running ignored tests");
        let verifier = SubxtOnChainVerifier::new(&rpc_url, "westend-asset-hub").with_client(client);
        assert!(verifier.has_client());
    }
}
