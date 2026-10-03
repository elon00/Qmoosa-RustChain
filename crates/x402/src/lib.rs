use parity_scale_codec::{Decode, Encode};
use qmoosa_primitives::AccountId32;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
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
    #[error("Recipient mismatch: expected {expected}, got {actual}")]
    RecipientMismatch { expected: String, actual: String },
    #[error("Asset mismatch: expected {expected}, got {actual}")]
    AssetMismatch { expected: String, actual: String },
    #[error("Insufficient payment: expected at least {expected}, got {actual}")]
    InsufficientPayment { expected: u128, actual: u128 },
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
    pub block_number: u64,
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

/// Production Polkadot RPC verifier definition
pub struct PolkadotRpcVerifier {
    pub rpc_url: String,
    pub expected_network_id: String,
}

impl PolkadotRpcVerifier {
    pub fn new(rpc_url: &str, expected_network_id: &str) -> Self {
        Self {
            rpc_url: rpc_url.to_string(),
            expected_network_id: expected_network_id.to_string(),
        }
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

        // 6. Recipient address check
        if onchain.recipient != self.merchant_address {
            return Err(X402Error::RecipientMismatch {
                expected: self.merchant_address.to_string(),
                actual: onchain.recipient.to_string(),
            });
        }

        // 7. Asset check
        if onchain.asset != challenge.asset {
            return Err(X402Error::AssetMismatch {
                expected: challenge.asset.clone(),
                actual: onchain.asset,
            });
        }

        // 8. Amount check (must be >= requested challenge amount)
        if onchain.amount < challenge.amount {
            return Err(X402Error::InsufficientPayment {
                expected: challenge.amount,
                actual: onchain.amount,
            });
        }

        // 9. Mark challenge & tx hash as settled
        {
            let mut c_guard = SETTLED_CHALLENGES.lock().unwrap();
            if let Some(set) = c_guard.as_mut() {
                set.insert(proof.challenge_id.clone());
            }

            let mut t_guard = SETTLED_TX_HASHES.lock().unwrap();
            if let Some(set) = t_guard.as_mut() {
                set.insert(proof.tx_hash.clone());
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
            block_number: 104523,
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
            block_number: 104524,
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
            block_number: 104525,
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
            block_number: 104526,
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
            block_number: 104527,
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
}
