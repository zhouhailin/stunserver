//! STUN integrity primitives.
//!
//! Implements MESSAGE-INTEGRITY (RFC 5389 section 15.4), FINGERPRINT
//! (RFC 5389 section 15.5), and long-term credential key derivation
//! (RFC 5389 section 10.2.2) with full SASLprep (RFC 4013).

use crate::error::StunError;
use hmac::{Hmac, Mac};
use md5::{Digest, Md5};
use sha1::Sha1;
use subtle::ConstantTimeEq;

/// Length of a MESSAGE-INTEGRITY value in bytes.
pub const MESSAGE_INTEGRITY_LEN: usize = 20;

/// Length of a FINGERPRINT value in bytes.
pub const FINGERPRINT_LEN: usize = 4;

/// Value XORed into the CRC-32 result to form a FINGERPRINT.
pub const FINGERPRINT_XOR: u32 = 0x5354_554E;

type HmacSha1 = Hmac<Sha1>;

/// Compute an HMAC-SHA1 digest.
pub fn hmac_sha1(key: &[u8], data: &[u8]) -> [u8; MESSAGE_INTEGRITY_LEN] {
    let mut mac = HmacSha1::new_from_slice(key).expect("HMAC accepts keys of any length");
    mac.update(data);
    mac.finalize().into_bytes().into()
}

/// Verify an HMAC-SHA1 digest in constant time.
///
/// A digest whose length is not 20 bytes is rejected.
pub fn verify_hmac_sha1(key: &[u8], data: &[u8], expected: &[u8]) -> bool {
    if expected.len() != MESSAGE_INTEGRITY_LEN {
        return false;
    }
    let computed = hmac_sha1(key, data);
    computed.ct_eq(expected).into()
}

/// Compute the FINGERPRINT value for the covered message bytes.
pub fn fingerprint(data: &[u8]) -> u32 {
    crc32fast::hash(data) ^ FINGERPRINT_XOR
}

/// Verify a FINGERPRINT value in constant time.
///
/// A value whose length is not 4 bytes is rejected.
pub fn verify_fingerprint(data: &[u8], expected: &[u8]) -> bool {
    if expected.len() != FINGERPRINT_LEN {
        return false;
    }
    let value = u32::from_be_bytes([expected[0], expected[1], expected[2], expected[3]]);
    fingerprint(data).ct_eq(&value).into()
}

/// Build the MESSAGE-INTEGRITY input range from a message.
///
/// The input is every byte before the MESSAGE-INTEGRITY attribute, with the
/// header length field rewritten so the message ends at the
/// MESSAGE-INTEGRITY attribute (RFC 5389 section 15.4).
///
/// # Panics
///
/// Panics when `mi_offset` is smaller than the STUN header.
pub fn hmac_input(message: &[u8], mi_offset: usize) -> Vec<u8> {
    assert!(mi_offset >= crate::message::HEADER_SIZE);
    let mut input = message[..mi_offset].to_vec();
    let adjusted = (mi_offset - crate::message::HEADER_SIZE + 4 + MESSAGE_INTEGRITY_LEN) as u16;
    input[2..4].copy_from_slice(&adjusted.to_be_bytes());
    input
}

/// Apply SASLprep (RFC 4013) to a credential string.
///
/// RFC 5389 sections 15.3, 15.4, and 15.7 require this preparation for
/// USERNAME, passwords, and REALM respectively.
pub fn saslprep(value: &str) -> Result<String, StunError> {
    stringprep::saslprep(value)
        .map(|prepared| prepared.into_owned())
        .map_err(|e| StunError::MalformedMessage(format!("SASLprep failed: {e}")))
}

/// Remove the credential decorations that RFC 5389 section 15.4 excludes from
/// key derivation: quotes and trailing nulls from USERNAME and REALM, trailing
/// nulls only from the password.
fn strip_credential(value: &str, strip_quotes: bool) -> &str {
    let value = value.trim_end_matches('\0');
    if strip_quotes {
        value.trim_matches('"')
    } else {
        value
    }
}

/// Derive the long-term MESSAGE-INTEGRITY key.
///
/// The key is `MD5(username ":" realm ":" SASLprep(password))`, where USERNAME
/// and REALM lose any quotes and trailing nulls before SASLprep (RFC 5389
/// sections 10.2.2, 15.3, 15.4, and 15.7).
pub fn long_term_key(username: &str, realm: &str, password: &str) -> Result<[u8; 16], StunError> {
    let username = saslprep(strip_credential(username, true))?;
    let realm = saslprep(strip_credential(realm, true))?;
    let password = saslprep(strip_credential(password, false))?;
    let mut hasher = Md5::new();
    hasher.update(username.as_bytes());
    hasher.update(b":");
    hasher.update(realm.as_bytes());
    hasher.update(b":");
    hasher.update(password.as_bytes());
    Ok(hasher.finalize().into())
}

/// Short-term MESSAGE-INTEGRITY keys are the SASLprep-prepared password bytes
/// (RFC 5389 section 15.4).
pub fn short_term_key(password: &str) -> Result<Vec<u8>, StunError> {
    saslprep(password).map(String::into_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hmac_input_adjusts_length() {
        let mut message = vec![0u8; 20];
        message[2..4].copy_from_slice(&40u16.to_be_bytes());
        message.extend_from_slice(&[0xAA; 16]);

        let input = hmac_input(&message, 36);
        assert_eq!(input.len(), 36);
        assert_eq!(u16::from_be_bytes([input[2], input[3]]), 40);
    }
}
