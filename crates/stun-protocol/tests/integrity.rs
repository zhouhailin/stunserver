//! Tests for the STUN integrity primitives.
//!
//! Covers HMAC-SHA1 MESSAGE-INTEGRITY, CRC-32 FINGERPRINT, long-term key
//! derivation with SASLprep, and constant-time digest comparison.

use stun_protocol::integrity::{
    fingerprint, hmac_sha1, long_term_key, saslprep, short_term_key, verify_fingerprint,
    verify_hmac_sha1, FINGERPRINT_XOR,
};

#[test]
fn test_hmac_sha1_known_vector() {
    let digest = hmac_sha1(b"key", b"The quick brown fox jumps over the lazy dog");
    assert_eq!(
        digest,
        [
            0xde, 0x7c, 0x9b, 0x85, 0xb8, 0xb7, 0x8a, 0xa6, 0xbc, 0x8a, 0x7a, 0x36, 0xf7, 0x0a,
            0x90, 0x70, 0x1c, 0x9d, 0xb4, 0xd9,
        ]
    );
    assert_eq!(digest.len(), 20);
}

#[test]
fn test_verify_hmac_sha1_rejects_tampering() {
    let key = b"VOkJxbRl1RmTxUk/WvJxBt";
    let data = b"authenticated bytes";
    let digest = hmac_sha1(key, data);

    assert!(verify_hmac_sha1(key, data, &digest));

    let mut tampered = digest;
    tampered[7] ^= 0x01;
    assert!(!verify_hmac_sha1(key, data, &tampered));

    let mut tampered_data = data.to_vec();
    tampered_data[0] ^= 0x01;
    assert!(!verify_hmac_sha1(key, &tampered_data, &digest));

    assert!(!verify_hmac_sha1(b"wrong-key", data, &digest));
    assert!(!verify_hmac_sha1(key, data, &digest[..19]));
}

#[test]
fn test_fingerprint_crc_xor() {
    // CRC-32 (IEEE) of "123456789" is 0xCBF43926; FINGERPRINT XORs 0x5354554E.
    assert_eq!(fingerprint(b"123456789"), 0xCBF43926 ^ FINGERPRINT_XOR);
}

#[test]
fn test_verify_fingerprint() {
    let data = b"message covered by the CRC";
    let value = fingerprint(data).to_be_bytes();
    assert!(verify_fingerprint(data, &value));

    let mut bad = value;
    bad[3] ^= 0x01;
    assert!(!verify_fingerprint(data, &bad));
    assert!(!verify_fingerprint(data, &value[..3]));
}

#[test]
fn test_long_term_key_derivation() {
    // MD5("user:realm:pass"), computed independently with hashlib.
    let key = long_term_key("user", "realm", "pass").expect("derive key");
    assert_eq!(
        key,
        [
            0x84, 0x93, 0xfb, 0xc5, 0x3b, 0xa5, 0x82, 0xfb, 0x4c, 0x04, 0x4c, 0x45, 0x6b, 0xdc,
            0x40, 0xeb,
        ]
    );
    assert_eq!(key.len(), 16);
}

#[test]
fn test_long_term_key_uses_saslprep_for_non_ascii() {
    // MD5("マトリックス:example.org:TheMatrIX") from RFC 5769 section 2.4.
    let key = long_term_key("マトリックス", "example.org", "TheMatrIX").expect("derive key");
    assert_eq!(
        key,
        [
            0xe8, 0xca, 0x7a, 0xd5, 0x9d, 0x5e, 0xb0, 0x51, 0x8e, 0x31, 0x29, 0x11, 0xd2, 0xda,
            0xb2, 0xa9,
        ]
    );

    // A password that only differs by characters SASLprep removes must
    // derive the same key.
    let composed = long_term_key("user", "realm", "pa\u{00AD}ss").expect("derive key");
    let plain = long_term_key("user", "realm", "pass").expect("derive key");
    assert_eq!(composed, plain);
}

#[test]
fn test_short_term_key_is_raw_password_bytes() {
    assert_eq!(short_term_key("s3cret"), b"s3cret".to_vec());
    assert_eq!(short_term_key(""), Vec::<u8>::new());
}

#[test]
fn test_saslprep_mapping() {
    // Non-breaking space maps to a plain space.
    assert_eq!(saslprep("a\u{00A0}b").unwrap(), "a b");
    // Soft hyphen maps to nothing.
    assert_eq!(saslprep("pa\u{00AD}ss").unwrap(), "pass");
}

#[test]
fn test_saslprep_normalization() {
    // NFKC: ROMAN NUMERAL FOUR becomes "IV"; superscript digits become ASCII.
    assert_eq!(saslprep("\u{2163}").unwrap(), "IV");
    assert_eq!(saslprep("\u{00B2}").unwrap(), "2");
}

#[test]
fn test_saslprep_rejects_prohibited_output() {
    assert!(saslprep("bad\u{0007}control").is_err());
    assert!(saslprep("bell\u{0007}").is_err());
}

#[test]
fn test_saslprep_rejects_bidirectional_violation() {
    // Hebrew letter mixed with Latin text violates the bidirectional rules.
    assert!(saslprep("abc\u{05D0}").is_err());
}

#[test]
fn test_saslprep_failure_propagates_from_key_derivation() {
    assert!(long_term_key("user", "realm", "bad\u{0007}").is_err());
}
