use fips204::ml_dsa_65;
use fips204::traits::{KeyGen, SerDes, Signer, Verifier};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};
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

impl Default for PqcProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl PqcProvider {
    pub fn new() -> Self {
        let (pk, sk) = ml_dsa_65::KG::try_keygen().unwrap();
        Self {
            public_key: pk.into_bytes().to_vec(),
            secret_key: sk.into_bytes().to_vec(),
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

        let sk_array: [u8; 4032] = self.secret_key.as_slice().try_into().unwrap();
        let sk = ml_dsa_65::PrivateKey::try_from_bytes(sk_array).unwrap();
        let sig = sk.try_sign(sign_target.as_bytes(), b"").unwrap();
        let signature_bytes = sig.to_vec();

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

    pub fn verify_envelope(
        payload: &str,
        envelope: &PqcSignatureEnvelope,
        check_replay: bool,
    ) -> Result<(), PqcError> {
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

        // 4. Genuine NIST FIPS 204 ML-DSA-65 cryptographic verification
        let pk_bytes =
            hex::decode(&envelope.public_key_hex).map_err(|_| PqcError::InvalidSignature)?;
        let pk_array: [u8; 1952] = pk_bytes
            .as_slice()
            .try_into()
            .map_err(|_| PqcError::InvalidSignature)?;
        let pk = ml_dsa_65::PublicKey::try_from_bytes(pk_array)
            .map_err(|_| PqcError::InvalidSignature)?;

        let sig_bytes =
            hex::decode(&envelope.signature_hex).map_err(|_| PqcError::InvalidSignature)?;
        let sig_array: [u8; 3309] = sig_bytes
            .as_slice()
            .try_into()
            .map_err(|_| PqcError::InvalidSignature)?;

        let sign_target = format!(
            "{}:{}:{}:{}",
            envelope.payload_digest, envelope.timestamp, envelope.expires_at, envelope.nonce
        );
        if !pk.verify(sign_target.as_bytes(), &sig_array, b"") {
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
    fn test_wrong_public_key_fail() {
        let provider1 = PqcProvider::new();
        let provider2 = PqcProvider::new();
        let payload = r#"{"action":"vote","proposal":42}"#;
        let mut envelope = provider1.sign_payload(payload, 5000);
        envelope.public_key_hex = provider2.public_key_hex(); // Substitute public key
        assert_eq!(
            PqcProvider::verify_envelope(payload, &envelope, false),
            Err(PqcError::InvalidSignature)
        );
    }

    #[test]
    fn test_corrupted_signature_fail() {
        let provider = PqcProvider::new();
        let payload = r#"{"action":"vote","proposal":42}"#;
        let mut envelope = provider.sign_payload(payload, 5000);
        let mut sig_bytes = hex::decode(&envelope.signature_hex).unwrap();
        // Flip a byte in the 3309-byte ML-DSA-65 signature
        sig_bytes[10] ^= 0xFF;
        envelope.signature_hex = hex::encode(sig_bytes);
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
