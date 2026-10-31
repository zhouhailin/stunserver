//! STUN authentication stage (RFC 5389 §10, §15).
//!
//! Supports short-term and long-term credential mechanisms. Long-term
//! challenges carry stateless NONCE values: 8 big-endian seconds followed by a
//! 16-byte truncated HMAC-SHA1 over those seconds, rendered as 48 lowercase
//! hex characters. Passwords, derived keys, and the nonce secret are never
//! rendered by `Debug` or error text.

use crate::config::{AuthConfig, AuthMode, AuthTable, ConfigError};
use rand::rngs::OsRng;
use rand::RngCore;
use std::fmt;
use stun_protocol::integrity::{hmac_sha1, verify_hmac_sha1};
use stun_protocol::{StunAttributeType, StunMessage};

/// Raw NONCE length in bytes: 8-byte timestamp plus 16-byte truncated HMAC.
pub const NONCE_BYTES: usize = 24;
/// Hex-rendered NONCE length.
pub const NONCE_HEX_LEN: usize = 48;
/// Truncated HMAC length inside a NONCE.
pub const NONCE_MAC_LEN: usize = 16;
/// Tolerance for NONCE timestamps that appear to come from the future.
const NONCE_FUTURE_SKEW_SECS: u64 = 60;

/// Result of verifying a NONCE.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NonceStatus {
    Fresh,
    Stale,
    Invalid,
}

/// Authentication decision for one request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthOutcome {
    /// Authentication is disabled; the response is not signed.
    Disabled,
    /// The request authenticated; the response is signed with this key.
    Authenticated(Vec<u8>),
    /// The request failed authentication.
    Rejected {
        status: u16,
        /// When true the error response carries REALM and NONCE.
        challenge: bool,
        reason: &'static str,
    },
}

impl AuthOutcome {
    fn rejected(status: u16, challenge: bool, reason: &'static str) -> Self {
        Self::Rejected {
            status,
            challenge,
            reason,
        }
    }
}

/// The authentication service: precomputed credentials plus the nonce secret.
#[derive(Clone)]
pub struct AuthService {
    table: AuthTable,
    nonce_secret: [u8; 32],
}

impl fmt::Debug for AuthService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuthService")
            .field("table", &self.table)
            .finish_non_exhaustive()
    }
}

impl AuthService {
    /// Build the service from the `[auth]` configuration, deriving every key
    /// once and generating a random NONCE secret.
    pub fn from_config(auth: &AuthConfig) -> Result<Self, ConfigError> {
        let table = auth.precompute_keys()?;
        let mut nonce_secret = [0u8; 32];
        OsRng.fill_bytes(&mut nonce_secret);
        Ok(Self {
            table,
            nonce_secret,
        })
    }

    /// A service with authentication disabled (used by the public path).
    pub fn disabled() -> Self {
        let table = AuthConfig::default()
            .precompute_keys()
            .expect("default auth config is valid");
        Self {
            table,
            nonce_secret: [0u8; 32],
        }
    }

    pub fn mode(&self) -> AuthMode {
        self.table.mode()
    }

    pub fn realm(&self) -> &str {
        self.table.realm()
    }

    /// Issue a NONCE stamped with the current time.
    pub fn issue_nonce(&self) -> String {
        self.issue_nonce_at(unix_now())
    }

    /// Issue a NONCE stamped with an explicit time (seconds since the epoch).
    pub fn issue_nonce_at(&self, now_secs: u64) -> String {
        let timestamp = now_secs.to_be_bytes();
        let mac = hmac_sha1(&self.nonce_secret, &timestamp);
        let mut bytes = [0u8; NONCE_BYTES];
        bytes[..8].copy_from_slice(&timestamp);
        bytes[8..].copy_from_slice(&mac[..NONCE_MAC_LEN]);
        hex_encode(&bytes)
    }

    /// Verify a NONCE against the current time.
    pub fn verify_nonce(&self, nonce: &str) -> NonceStatus {
        self.verify_nonce_at(nonce, unix_now())
    }

    /// Verify a NONCE against an explicit time.
    pub fn verify_nonce_at(&self, nonce: &str, now_secs: u64) -> NonceStatus {
        if nonce.len() != NONCE_HEX_LEN {
            return NonceStatus::Invalid;
        }
        let Some(bytes) = hex_decode(nonce) else {
            return NonceStatus::Invalid;
        };

        let expected = hmac_sha1(&self.nonce_secret, &bytes[..8]);
        if !constant_time_eq(&bytes[8..], &expected[..NONCE_MAC_LEN]) {
            return NonceStatus::Invalid;
        }

        let timestamp = u64::from_be_bytes(bytes[..8].try_into().expect("8-byte timestamp"));
        if timestamp > now_secs.saturating_add(NONCE_FUTURE_SKEW_SECS) {
            return NonceStatus::Invalid;
        }
        if now_secs.saturating_sub(timestamp) > self.table.nonce_lifetime_sec() {
            return NonceStatus::Stale;
        }
        NonceStatus::Fresh
    }

    /// Run the authentication stage for the configured mode.
    pub fn authenticate(&self, encoded: &[u8], message: &StunMessage) -> AuthOutcome {
        match self.table.mode() {
            AuthMode::Disabled => AuthOutcome::Disabled,
            AuthMode::ShortTerm => self.authenticate_short_term(encoded, message),
            AuthMode::LongTerm => self.authenticate_long_term(encoded, message),
        }
    }

    fn authenticate_short_term(&self, encoded: &[u8], message: &StunMessage) -> AuthOutcome {
        let mi_present = message.attribute_count(StunAttributeType::MessageIntegrity as u16) == 1;
        let Some(username) = text_attribute(message, StunAttributeType::Username) else {
            return AuthOutcome::rejected(400, false, "missing USERNAME");
        };
        if !mi_present {
            return AuthOutcome::rejected(400, false, "missing MESSAGE-INTEGRITY");
        }

        let Some(key) = self.table.key_for(username) else {
            return AuthOutcome::rejected(401, false, "unknown USERNAME");
        };
        if !verify_integrity(encoded, message, key) {
            return AuthOutcome::rejected(401, false, "MESSAGE-INTEGRITY mismatch");
        }
        AuthOutcome::Authenticated(key.to_vec())
    }

    fn authenticate_long_term(&self, encoded: &[u8], message: &StunMessage) -> AuthOutcome {
        let mi_present = message.attribute_count(StunAttributeType::MessageIntegrity as u16) == 1;
        if !mi_present {
            return AuthOutcome::rejected(401, true, "missing MESSAGE-INTEGRITY");
        }

        let username = text_attribute(message, StunAttributeType::Username);
        let realm = text_attribute(message, StunAttributeType::Realm);
        let nonce = text_attribute(message, StunAttributeType::Nonce);
        let (Some(username), Some(realm), Some(nonce)) = (username, realm, nonce) else {
            return AuthOutcome::rejected(400, false, "missing long-term attributes");
        };

        if self.verify_nonce(nonce) != NonceStatus::Fresh {
            return AuthOutcome::rejected(438, true, "stale or invalid NONCE");
        }

        if realm != self.table.realm() {
            return AuthOutcome::rejected(401, true, "REALM mismatch");
        }

        let Some(key) = self.table.key_for(username) else {
            return AuthOutcome::rejected(401, true, "unknown USERNAME");
        };
        if !verify_integrity(encoded, message, key) {
            return AuthOutcome::rejected(401, true, "MESSAGE-INTEGRITY mismatch");
        }
        AuthOutcome::Authenticated(key.to_vec())
    }
}

/// Read a UTF-8 attribute that appears before MESSAGE-INTEGRITY. Attributes
/// after MESSAGE-INTEGRITY are ignored for authentication.
fn text_attribute(message: &StunMessage, attribute_type: StunAttributeType) -> Option<&str> {
    for attribute in &message.attributes {
        if attribute.raw_type == StunAttributeType::MessageIntegrity as u16 {
            break;
        }
        if attribute.raw_type == attribute_type as u16 {
            return std::str::from_utf8(&attribute.attribute_value)
                .ok()
                .filter(|text| !text.is_empty());
        }
    }
    None
}

fn verify_integrity(encoded: &[u8], message: &StunMessage, key: &[u8]) -> bool {
    let Some(index) = message
        .attributes
        .iter()
        .position(|attr| attr.raw_type == StunAttributeType::MessageIntegrity as u16)
    else {
        return false;
    };
    let offset = message.attribute_offsets()[index];
    let input = stun_protocol::integrity::hmac_input(encoded, offset);
    verify_hmac_sha1(key, &input, &message.attributes[index].attribute_value)
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn hex_decode(text: &str) -> Option<[u8; NONCE_BYTES]> {
    let bytes = text.as_bytes();
    if bytes.len() != NONCE_HEX_LEN {
        return None;
    }
    let mut out = [0u8; NONCE_BYTES];
    for (index, pair) in bytes.as_chunks::<2>().0.iter().enumerate() {
        let high = (pair[0] as char).to_digit(16)?;
        let low = (pair[1] as char).to_digit(16)?;
        out[index] = ((high << 4) | low) as u8;
    }
    Some(out)
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b) {
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AuthConfig;

    fn short_term_service() -> AuthService {
        let auth: AuthConfig = toml::from_str(
            r#"
mode = "short-term"

[[users]]
username = "alice"
password = "s3cret"
"#,
        )
        .unwrap();
        AuthService::from_config(&auth).unwrap()
    }

    #[test]
    fn test_nonce_round_trip_is_fresh() {
        let service = short_term_service();
        let nonce = service.issue_nonce_at(1_000);
        assert_eq!(nonce.len(), NONCE_HEX_LEN);
        assert!(nonce
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)));
        assert_eq!(service.verify_nonce_at(&nonce, 1_000), NonceStatus::Fresh);
    }

    #[test]
    fn test_nonce_tamper_is_invalid() {
        let service = short_term_service();
        let nonce = service.issue_nonce_at(1_000);
        let tampered: String = nonce
            .chars()
            .enumerate()
            .map(|(i, c)| if i == 20 { '0' } else { c })
            .collect();
        assert_eq!(
            service.verify_nonce_at(&tampered, 1_000),
            NonceStatus::Invalid
        );
        assert_eq!(
            service.verify_nonce_at("not-a-nonce", 1_000),
            NonceStatus::Invalid
        );
    }

    #[test]
    fn test_nonce_expiry_is_stale() {
        let service = short_term_service();
        let nonce = service.issue_nonce_at(1_000);
        assert_eq!(
            service.verify_nonce_at(&nonce, 1_000 + 601),
            NonceStatus::Stale
        );
    }

    #[test]
    fn test_debug_hides_secrets() {
        let service = short_term_service();
        let rendered = format!("{:?}", service);
        assert!(!rendered.contains("s3cret"));
        assert!(!rendered.contains("nonce_secret"));
    }
}
