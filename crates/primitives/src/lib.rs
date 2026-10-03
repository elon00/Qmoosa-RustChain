use blake2::{Blake2b512, Digest};
use parity_scale_codec::{Decode, Encode};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use thiserror::Error;

pub const SS58_PREFIX_POLKADOT: u16 = 0;
pub const SS58_PREFIX_SUBSTRATE: u16 = 42;
const SS58_PREFIX_BYTES: &[u8] = b"SS58PRE";

#[derive(Error, Debug, PartialEq, Eq)]
pub enum Ss58Error {
    #[error("Base58 decoding error")]
    Base58DecodeError,
    #[error("Invalid address length: expected 35 bytes, got {0}")]
    InvalidLength(usize),
    #[error("Invalid SS58 checksum")]
    InvalidChecksum,
    #[error("Unsupported address prefix: {0}")]
    UnsupportedPrefix(u16),
}

/// Native 32-byte Account Identifier used across Polkadot and Substrate runtimes.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Encode, Decode)]
pub struct AccountId32(pub [u8; 32]);

impl AccountId32 {
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_hex(&self) -> String {
        format!("0x{}", hex::encode(self.0))
    }

    pub fn from_hex(s: &str) -> Result<Self, hex::FromHexError> {
        let clean = s.strip_prefix("0x").unwrap_or(s);
        let bytes = hex::decode(clean)?;
        if bytes.len() != 32 {
            return Err(hex::FromHexError::InvalidStringLength);
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(Self(arr))
    }

    /// Encodes this AccountId32 to an SS58 address string using the given network prefix.
    /// Prefix 0 = Polkadot Mainnet, Prefix 42 = Generic Substrate.
    pub fn to_ss58(&self, prefix: u16) -> String {
        let mut buffer = Vec::with_capacity(35);
        if prefix < 64 {
            buffer.push(prefix as u8);
        } else {
            buffer.push(((prefix & 0b0000_0000_1111_1100) as u8) | (((prefix >> 8) as u8) << 2));
            buffer.push((prefix >> 2) as u8);
        }
        buffer.extend_from_slice(&self.0);

        let mut hasher = Blake2b512::new();
        hasher.update(SS58_PREFIX_BYTES);
        hasher.update(&buffer);
        let checksum = hasher.finalize();

        buffer.extend_from_slice(&checksum[0..2]);
        bs58::encode(buffer).into_string()
    }

    /// Decodes an SS58 address string and returns the AccountId32 and its network prefix.
    pub fn from_ss58(ss58: &str) -> Result<(Self, u16), Ss58Error> {
        let decoded = bs58::decode(ss58)
            .into_vec()
            .map_err(|_| Ss58Error::Base58DecodeError)?;
        if decoded.len() != 35 {
            return Err(Ss58Error::InvalidLength(decoded.len()));
        }

        let prefix = decoded[0] as u16;
        let mut account_bytes = [0u8; 32];
        account_bytes.copy_from_slice(&decoded[1..33]);
        let provided_checksum = &decoded[33..35];

        let mut hasher = Blake2b512::new();
        hasher.update(SS58_PREFIX_BYTES);
        hasher.update(&decoded[0..33]);
        let calculated_checksum = hasher.finalize();

        if provided_checksum != &calculated_checksum[0..2] {
            return Err(Ss58Error::InvalidChecksum);
        }

        Ok((Self(account_bytes), prefix))
    }
}

impl fmt::Debug for AccountId32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AccountId32({})", self.to_ss58(SS58_PREFIX_SUBSTRATE))
    }
}

impl fmt::Display for AccountId32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_ss58(SS58_PREFIX_SUBSTRATE))
    }
}

impl Serialize for AccountId32 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_ss58(SS58_PREFIX_SUBSTRATE))
    }
}

impl<'de> Deserialize<'de> for AccountId32 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        if s.starts_with("0x") {
            Self::from_hex(&s).map_err(serde::de::Error::custom)
        } else {
            Self::from_ss58(&s)
                .map(|(acc, _)| acc)
                .map_err(serde::de::Error::custom)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ss58_encode_decode_roundtrip() {
        let raw = [7u8; 32];
        let account = AccountId32::new(raw);

        let ss58_generic = account.to_ss58(SS58_PREFIX_SUBSTRATE);
        assert!(ss58_generic.starts_with('5'));

        let (decoded, prefix) = AccountId32::from_ss58(&ss58_generic).unwrap();
        assert_eq!(decoded, account);
        assert_eq!(prefix, SS58_PREFIX_SUBSTRATE);

        let ss58_polkadot = account.to_ss58(SS58_PREFIX_POLKADOT);
        assert!(ss58_polkadot.starts_with('1'));

        let (decoded_dot, prefix_dot) = AccountId32::from_ss58(&ss58_polkadot).unwrap();
        assert_eq!(decoded_dot, account);
        assert_eq!(prefix_dot, SS58_PREFIX_POLKADOT);
    }

    #[test]
    fn test_user_wallet_ss58_decoding() {
        // User's Substrate wallet address: 5CwB8yg9VmeHX1CCYbzrNZWoj1uTLcVqGHgqBAasXetnQwnd
        let user_addr = "5CwB8yg9VmeHX1CCYbzrNZWoj1uTLcVqGHgqBAasXetnQwnd";
        let res = AccountId32::from_ss58(user_addr);
        assert!(
            res.is_ok(),
            "User wallet SS58 address must decode cleanly with valid checksum"
        );
        let (acc, prefix) = res.unwrap();
        assert_eq!(prefix, SS58_PREFIX_SUBSTRATE);
        assert_eq!(acc.to_ss58(SS58_PREFIX_SUBSTRATE), user_addr);
    }

    #[test]
    fn test_ss58_corrupted_checksum_fails() {
        let raw = [1u8; 32];
        let account = AccountId32::new(raw);
        let ss58 = account.to_ss58(SS58_PREFIX_SUBSTRATE);

        // Tamper with the last character
        let mut chars: Vec<char> = ss58.chars().collect();
        let last = chars.len() - 1;
        chars[last] = if chars[last] == 'a' { 'b' } else { 'a' };
        let corrupted: String = chars.into_iter().collect();

        let res = AccountId32::from_ss58(&corrupted);
        assert_eq!(res, Err(Ss58Error::InvalidChecksum));
    }

    #[test]
    fn test_scale_codec_roundtrip() {
        let account = AccountId32::new([42u8; 32]);
        let encoded = account.encode();
        assert_eq!(encoded.len(), 32);

        let decoded = AccountId32::decode(&mut &encoded[..]).unwrap();
        assert_eq!(decoded, account);
    }
}
