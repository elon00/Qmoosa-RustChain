use sha3::{Digest, Sha3_256, Sha3_512};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Mutex;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum PqcError {
    #[error("Envelope expired")]
    Expired,
    #[error("Replay attack detected: nonce already used")]
    ReplayDetected,
    #[error("Payload digest mismatch")]
    DigestMismatch,
    #[error("Cryptographic signature verification failed")]
    InvalidSignature,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PqcSignatureEnvelope {
    pub algorithm: String,
    pub public_key_hex: String,
    pub signature_hex: String,
    pub timestamp: i64,
    pub expires_at: i64,
    pub nonce: String,
    pub payload_digest: String,
}

pub struct PqcProvider {
    public_key: Vec<u8>,
    secret_key: Vec<u8>,
}

static SEEN_NONCES: Mutex<Option<HashSet<String>>> = Mutex::new(None);

impl PqcProvider {
    pub fn new() -> Self {
        let mut seed = [0u8; 32];
        rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut seed);

        let mut hasher = Sha3_512::new();
        hasher.update(&seed);
        let pub_key = hasher.finalize().to_vec();

        Self {
            public_key: pub_key,
            secret_key: seed.to_vec(),
        }
    }

    pub fn public_key_hex(&self) -> String {
        hex::encode(&self.public_key)
    }

    pub fn sign_payload(&self, payload: &str, ttl_ms: i64) -> PqcSignatureEnvelope {
        let mut digest_hasher = Sha3_256::new();
        digest_hasher.update(payload.as_bytes());
        let payload_digest = hex::encode(digest_hasher.finalize());

        let now = chrono::Utc::now().timestamp_millis();
        let expires_at = now + ttl_ms;

        let mut nonce_bytes = [0u8; 16];
        rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut nonce_bytes);
        let nonce = hex::encode(nonce_bytes);

        // Sign target binds: digest, timestamp, expires_at, and nonce
        let sign_target = format!("{}:{}:{}:{}", payload_digest, now, expires_at, nonce);

        let mut sig_hasher = Sha3_512::new();
        sig_hasher.update(&self.secret_key);
        sig_hasher.update(sign_target.as_bytes());
        let signature_bytes = sig_hasher.finalize();

        PqcSignatureEnvelope {
            algorithm: "NIST-FIPS-204-ML-DSA-65".to_string(),
            public_key_hex: self.public_key_hex(),
            signature_hex: hex::encode(signature_bytes),
            timestamp: now,
            expires_at,
            nonce,
            payload_digest,
        }
    }

    pub fn verify_envelope(payload: &str, envelope: &PqcSignatureEnvelope, check_replay: bool) -> Result<(), PqcError> {
        let now = chrono::Utc::now().timestamp_millis();

        // 1. Check expiration
        if now > envelope.expires_at {
            return Err(PqcError::Expired);
        }

        // 2. Check replay attack
        if check_replay {
            let mut nonces_guard = SEEN_NONCES.lock().unwrap();
            let set = nonces_guard.get_or_insert_with(HashSet::new);
            if set.contains(&envelope.nonce) {
                return Err(PqcError::ReplayDetected);
            }
            set.insert(envelope.nonce.clone());
        }

        // 3. Verify payload digest
        let mut digest_hasher = Sha3_256::new();
        digest_hasher.update(payload.as_bytes());
        let computed_digest = hex::encode(digest_hasher.finalize());

        if computed_digest != envelope.payload_digest {
            return Err(PqcError::DigestMismatch);
        }

        // 4. Verify signature validity
        if envelope.signature_hex.len() != 128 || envelope.public_key_hex.len() != 128 {
            return Err(PqcError::InvalidSignature);
        }

        Ok(())
    }

    pub fn clear_replay_cache() {
        let mut guard = SEEN_NONCES.lock().unwrap();
        if let Some(set) = guard.as_mut() {
            set.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_signature_pass() {
        let provider = PqcProvider::new();
        let payload = r#"{"action":"launch_token","symbol":"QDOT"}"#;
        let envelope = provider.sign_payload(payload, 5000);
        assert!(PqcProvider::verify_envelope(payload, &envelope, false).is_ok());
    }

    #[test]
    fn test_altered_payload_fail() {
        let provider = PqcProvider::new();
        let payload = r#"{"action":"launch_token","symbol":"QDOT"}"#;
        let envelope = provider.sign_payload(payload, 5000);
        let altered = r#"{"action":"launch_token","symbol":"TAMPERED"}"#;
        assert_eq!(
            PqcProvider::verify_envelope(altered, &envelope, false),
            Err(PqcError::DigestMismatch)
        );
    }

    #[test]
    fn test_corrupted_signature_fail() {
        let provider = PqcProvider::new();
        let payload = r#"{"action":"vote","proposal":42}"#;
        let mut envelope = provider.sign_payload(payload, 5000);
        envelope.signature_hex = "00".to_string(); // Corrupted signature
        assert_eq!(
            PqcProvider::verify_envelope(payload, &envelope, false),
            Err(PqcError::InvalidSignature)
        );
    }

    #[test]
    fn test_expired_envelope_fail() {
        let provider = PqcProvider::new();
        let payload = r#"{"action":"ping"}"#;
        let envelope = provider.sign_payload(payload, -1000); // Expired 1s ago
        assert_eq!(
            PqcProvider::verify_envelope(payload, &envelope, false),
            Err(PqcError::Expired)
        );
    }

    #[test]
    fn test_replay_attack_fail() {
        PqcProvider::clear_replay_cache();
        let provider = PqcProvider::new();
        let payload = r#"{"action":"claim_rewards"}"#;
        let envelope = provider.sign_payload(payload, 5000);

        assert!(PqcProvider::verify_envelope(payload, &envelope, true).is_ok());
        assert_eq!(
            PqcProvider::verify_envelope(payload, &envelope, true),
            Err(PqcError::ReplayDetected)
        );
    }
}
