//! Tests for ERROR-CODE and UNKNOWN-ATTRIBUTES value codecs (RFC 5389 §15.6, §15.9).

use stun_protocol::{ErrorCode, StunError, UnknownAttributes};

#[test]
fn test_error_code_roundtrip_420() {
    let code = ErrorCode::new(4, 20, "Unknown Attribute".to_string());
    let value = code.encode_value();

    // 4-byte header + 17-byte reason; padding is added by the attribute encoder.
    assert_eq!(value.len(), 21);
    assert_eq!(&value[0..4], &[0x00, 0x00, 0x04, 0x14]);
    assert_eq!(&value[4..21], b"Unknown Attribute");

    let decoded = ErrorCode::decode_value(&value).unwrap();
    assert_eq!(decoded.class, 4);
    assert_eq!(decoded.number, 20);
    assert_eq!(decoded.reason, "Unknown Attribute");
    assert_eq!(decoded.status_code(), 420);
}

#[test]
fn test_error_code_class_masked_to_three_bits() {
    let value = [0x00, 0x00, 0xFC, 0x05];
    let decoded = ErrorCode::decode_value(&value).unwrap();
    assert_eq!(decoded.class, 4);
    assert_eq!(decoded.number, 5);
}

#[test]
fn test_error_code_too_short_rejected() {
    let err = ErrorCode::decode_value(&[0x00, 0x00, 0x04]).unwrap_err();
    assert!(matches!(err, StunError::MalformedMessage(_)));
}

#[test]
fn test_unknown_attributes_single_roundtrip() {
    let unknown = UnknownAttributes::new(vec![0x0003]);
    let value = unknown.encode_value();
    // Raw value is two bytes; the attribute encoder adds two padding bytes.
    assert_eq!(value, vec![0x00, 0x03]);
    assert_eq!(
        UnknownAttributes::decode_value(&value).unwrap().types,
        vec![0x0003]
    );
}

#[test]
fn test_unknown_attributes_multiple_roundtrip() {
    let unknown = UnknownAttributes::new(vec![0x0024, 0x0025]);
    let value = unknown.encode_value();
    assert_eq!(value, vec![0x00, 0x24, 0x00, 0x25]);
    assert_eq!(
        UnknownAttributes::decode_value(&value).unwrap().types,
        vec![0x0024, 0x0025]
    );
}

#[test]
fn test_unknown_attributes_deduplicates_on_encode() {
    let unknown = UnknownAttributes::new(vec![0x0024, 0x0024, 0x0025, 0x0024]);
    let value = unknown.encode_value();
    assert_eq!(value, vec![0x00, 0x24, 0x00, 0x25]);
}

#[test]
fn test_unknown_attributes_odd_length_rejected() {
    let err = UnknownAttributes::decode_value(&[0x00, 0x24, 0x00]).unwrap_err();
    assert!(matches!(err, StunError::MalformedMessage(_)));
}

#[test]
fn test_unknown_attributes_empty() {
    let value = UnknownAttributes::new(Vec::new()).encode_value();
    assert!(value.is_empty());
    assert!(UnknownAttributes::decode_value(&value)
        .unwrap()
        .types
        .is_empty());
}
