use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Mutex;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum X402Error {
    #[error("Payment proof has expired")]
    Expired,
    #[error("Malformed transaction hash")]
    MalformedTxHash,
    #[error("Replay payment challenge detected")]
    ReplayDetected,
    #[error("Empty payment parameter")]
    InvalidParameters,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct X402Challenge {
    pub status_code: u16,
    pub challenge_id: String,
    pub pay_to_address: String,
    pub amount: String,
    pub asset: String,
    pub network: String,
    pub expires_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct X402Proof {
    pub challenge_id: String,
    pub tx_hash: String,
    pub payer_address: String,
}

pub struct X402BazaarGateway {
    pub merchant_address: String,
    pub default_amount: String,
    pub default_asset: String,
}

static SETTLED_CHALLENGES: Mutex<Option<HashSet<String>>> = Mutex::new(None);

impl X402BazaarGateway {
    pub fn new(merchant_address: &str, default_amount: &str, default_asset: &str) -> Self {
        Self {
            merchant_address: merchant_address.to_string(),
            default_amount: default_amount.to_string(),
            default_asset: default_asset.to_string(),
        }
    }

    pub fn generate_challenge(&self, custom_amount: Option<&str>) -> X402Challenge {
        let mut random_bytes = [0u8; 32];
        rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut random_bytes);
        let challenge_id = format!("0x{}", hex::encode(random_bytes));

        let now = chrono::Utc::now().timestamp_millis();
        let expires_at = now + 15 * 60 * 1000; // 15 mins

        X402Challenge {
            status_code: 402,
            challenge_id,
            pay_to_address: self.merchant_address.clone(),
            amount: custom_amount.unwrap_or(&self.default_amount).to_string(),
            asset: self.default_asset.clone(),
            network: "polkadot-asset-hub".to_string(),
            expires_at,
        }
    }

    pub fn verify_proof(&self, proof: &X402Proof) -> Result<bool, X402Error> {
        if proof.challenge_id.is_empty() || proof.tx_hash.is_empty() || proof.payer_address.is_empty() {
            return Err(X402Error::InvalidParameters);
        }

        // Validate transaction hash format
        if !proof.tx_hash.starts_with("0x") || proof.tx_hash.len() < 66 {
            return Err(X402Error::MalformedTxHash);
        }

        // Check replay protection
        let mut set_guard = SETTLED_CHALLENGES.lock().unwrap();
        let set = set_guard.get_or_insert_with(HashSet::new);
        if set.contains(&proof.challenge_id) {
            return Err(X402Error::ReplayDetected);
        }
        set.insert(proof.challenge_id.clone());

        Ok(true)
    }

    pub fn clear_cache() {
        let mut guard = SETTLED_CHALLENGES.lock().unwrap();
        if let Some(set) = guard.as_mut() {
            set.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_challenge() {
        let gw = X402BazaarGateway::new("5CwB8yg9VmeHX1CCYbzrNZWoj1uTLcVqGHgqBAasXetnQwnd", "0.05", "DOT");
        let challenge = gw.generate_challenge(None);
        assert_eq!(challenge.status_code, 402);
        assert!(challenge.challenge_id.starts_with("0x"));
        assert_eq!(challenge.amount, "0.05");
        assert_eq!(challenge.asset, "DOT");
    }

    #[test]
    fn test_valid_payment_proof_pass() {
        X402BazaarGateway::clear_cache();
        let gw = X402BazaarGateway::new("5CwB8yg9VmeHX1CCYbzrNZWoj1uTLcVqGHgqBAasXetnQwnd", "0.05", "DOT");
        let challenge = gw.generate_challenge(None);

        let proof = X402Proof {
            challenge_id: challenge.challenge_id,
            tx_hash: format!("0x{}", "a".repeat(64)),
            payer_address: "5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY".to_string(),
        };

        assert!(gw.verify_proof(&proof).unwrap());
    }

    #[test]
    fn test_malformed_tx_hash_fail() {
        let gw = X402BazaarGateway::new("5CwB8yg9VmeHX1CCYbzrNZWoj1uTLcVqGHgqBAasXetnQwnd", "0.05", "DOT");
        let proof = X402Proof {
            challenge_id: "0x123".to_string(),
            tx_hash: "0xshort".to_string(),
            payer_address: "5Grwva".to_string(),
        };

        assert_eq!(gw.verify_proof(&proof), Err(X402Error::MalformedTxHash));
    }

    #[test]
    fn test_replay_challenge_fail() {
        X402BazaarGateway::clear_cache();
        let gw = X402BazaarGateway::new("5CwB8yg9VmeHX1CCYbzrNZWoj1uTLcVqGHgqBAasXetnQwnd", "0.05", "DOT");
        let challenge = gw.generate_challenge(None);

        let proof = X402Proof {
            challenge_id: challenge.challenge_id,
            tx_hash: format!("0x{}", "b".repeat(64)),
            payer_address: "5Grwva".to_string(),
        };

        assert!(gw.verify_proof(&proof).unwrap());
        assert_eq!(gw.verify_proof(&proof), Err(X402Error::ReplayDetected));
    }
}
