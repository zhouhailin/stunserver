//! Tests for ERROR-CODE and UNKNOWN-ATTRIBUTES value codecs (RFC 5389 §15.6, §15.9).

use stun_protocol::{ErrorCode, StunError, UnknownAttributes};

#[test]
fn test_error_code_roundtrip_420() {
    let code = ErrorCode::new(4, 20, "Unknown Attribute".to_string()).unwrap();
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
fn test_error_code_new_validates_class_and_number() {
    // RFC 5389 section 15.6: class MUST be 3-6, number MUST be 0-99.
    assert!(ErrorCode::new(3, 0, String::new()).is_ok());
    assert!(ErrorCode::new(6, 99, String::new()).is_ok());
    assert!(ErrorCode::new(2, 0, String::new()).is_err());
    assert!(ErrorCode::new(7, 0, String::new()).is_err());
    assert!(ErrorCode::new(4, 100, String::new()).is_err());
}

#[test]
fn test_error_code_new_rejects_overlong_reason() {
    // RFC 5389 section 15.6: the reason phrase is under 128 characters.
    assert!(ErrorCode::new(4, 0, "x".repeat(127)).is_ok());
    assert!(ErrorCode::new(4, 0, "x".repeat(128)).is_err());
    assert!(ErrorCode::new(4, 0, "é".repeat(128)).is_err());
}

#[test]
fn test_error_code_decode_rejects_invalid_class_and_number() {
    let class_two = [0x00, 0x00, 0x02, 0x00];
    assert!(ErrorCode::decode_value(&class_two).is_err());

    let class_seven = [0x00, 0x00, 0x07, 0x00];
    assert!(ErrorCode::decode_value(&class_seven).is_err());

    let number_hundred = [0x00, 0x00, 0x04, 0x64];
    assert!(ErrorCode::decode_value(&number_hundred).is_err());
}

#[test]
fn test_error_code_decode_rejects_invalid_utf8_reason() {
    let value = [0x00, 0x00, 0x04, 0x00, 0xFF, 0xFE];
    assert!(ErrorCode::decode_value(&value).is_err());
}

#[test]
fn test_error_code_decode_rejects_overlong_reason() {
    let mut value = vec![0x00, 0x00, 0x04, 0x00];
    value.extend_from_slice("é".repeat(128).as_bytes());
    assert!(ErrorCode::decode_value(&value).is_err());

    let mut short = vec![0x00, 0x00, 0x04, 0x00];
    short.extend_from_slice("é".repeat(127).as_bytes());
    assert_eq!(
        ErrorCode::decode_value(&short)
            .unwrap()
            .reason
            .chars()
            .count(),
        127
    );
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
