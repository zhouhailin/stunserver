//! Authentication-focused protocol tests.
//!
//! Verifies MESSAGE-INTEGRITY verification, key derivation with and without
//! SASLprep normalization, tamper rejection, and the rule that attributes
//! after MESSAGE-INTEGRITY are excluded from the digest range.

use stun_protocol::integrity::{hmac_sha1, long_term_key, short_term_key, verify_hmac_sha1};
use stun_protocol::{StunAttribute, StunAttributeType, StunMessage, StunMessageType};

const TXN: [u8; 12] = [0x7C; 12];

fn signed_request(username: &str, key: &[u8]) -> Vec<u8> {
    let mut request = StunMessage::new(StunMessageType::Request, TXN);
    request.add_attribute(StunAttribute::new(
        StunAttributeType::Username,
        username.as_bytes().to_vec(),
    ));
    request.encode_with_integrity(key)
}

#[test]
fn test_short_term_hmac_verifies() {
    let key = short_term_key("s3cret");
    let bytes = signed_request("alice", &key);

    assert!(StunMessage::verify_message_integrity(&bytes, &key));
    assert!(!StunMessage::verify_message_integrity(&bytes, b"wrong"));
}

#[test]
fn test_tampered_request_fails_verification() {
    let key = short_term_key("s3cret");
    let mut bytes = signed_request("alice", &key);

    // Flip a byte inside USERNAME so the digest no longer matches.
    bytes[24] ^= 0x01;
    assert!(!StunMessage::verify_message_integrity(&bytes, &key));
}

#[test]
fn test_long_term_ascii_password_key() {
    let key = long_term_key("alice", "example.org", "change-me").expect("derive");
    let bytes = signed_request("alice", &key);

    assert_eq!(key.len(), 16);
    assert!(StunMessage::verify_message_integrity(&bytes, &key));
}

#[test]
fn test_long_term_non_ascii_password_saslprep() {
    // "pässword" with a combining diaeresis normalizes under NFKC; signing
    // with the precomposed spelling must yield the same key.
    let decomposed = "pa\u{0308}ssword";
    let composed = "pässword";
    let key_decomposed = long_term_key("bob", "example.org", decomposed).expect("derive");
    let key_composed = long_term_key("bob", "example.org", composed).expect("derive");
    assert_eq!(key_decomposed, key_composed);

    let bytes = signed_request("bob", &key_composed);
    assert!(StunMessage::verify_message_integrity(
        &bytes,
        &key_decomposed
    ));
}

#[test]
fn test_attributes_after_message_integrity_are_excluded() {
    let key = short_term_key("s3cret");
    let mut bytes = signed_request("alice", &key);

    // Append a SOFTWARE attribute after MESSAGE-INTEGRITY and update the
    // header length; the digest must still verify because the HMAC range
    // ends at MESSAGE-INTEGRITY.
    let attribute = StunAttribute::new(StunAttributeType::Software, b"trailing".to_vec()).encode();
    bytes.extend_from_slice(&attribute);
    let new_length = (bytes.len() - 20) as u16;
    bytes[2..4].copy_from_slice(&new_length.to_be_bytes());

    let decoded = StunMessage::decode(&bytes).expect("decode");
    assert_eq!(decoded.attributes.len(), 3);
    assert!(StunMessage::verify_message_integrity(&bytes, &key));
}

#[test]
fn test_manual_hmac_range_matches_encoding() {
    let key = short_term_key("s3cret");
    let bytes = signed_request("alice", &key);
    let decoded = StunMessage::decode(&bytes).unwrap();
    let mi_offset = decoded.attribute_offset(0x0008).unwrap();

    // Recompute independently: header up to MESSAGE-INTEGRITY with the length
    // field pointing at the end of MESSAGE-INTEGRITY.
    let mut input = bytes[..mi_offset].to_vec();
    input[2..4].copy_from_slice(&((mi_offset - 20 + 24) as u16).to_be_bytes());
    let expected = hmac_sha1(&key, &input);

    assert_eq!(
        &decoded.attributes.last().unwrap().attribute_value,
        &expected[..]
    );
    assert!(verify_hmac_sha1(&key, &input, &expected));
}
