//! Unit tests for STUN protocol library

use stun_protocol::*;

#[test]
fn test_message_type_conversion() {
    assert_eq!(StunMessageType::Request.to_raw(), 0x0001);
    assert_eq!(StunMessageType::Indication.to_raw(), 0x0011);
    assert_eq!(StunMessageType::Response.to_raw(), 0x0101);
    assert_eq!(StunMessageType::ErrorResponse.to_raw(), 0x0111);

    assert_eq!(
        StunMessageType::from_raw(0x0001).unwrap(),
        StunMessageType::Request
    );
    assert_eq!(
        StunMessageType::from_raw(0x0011).unwrap(),
        StunMessageType::Indication
    );
    assert_eq!(
        StunMessageType::from_raw(0x0101).unwrap(),
        StunMessageType::Response
    );
    assert_eq!(
        StunMessageType::from_raw(0x0111).unwrap(),
        StunMessageType::ErrorResponse
    );

    assert!(StunMessageType::from_raw(0x9999).is_err());
}

#[test]
fn test_message_creation() {
    let transaction_id = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
    let msg = StunMessage::new(StunMessageType::Request, transaction_id);

    assert_eq!(msg.message_type, StunMessageType::Request);
    assert_eq!(msg.magic_cookie, 0x2112A442);
    assert_eq!(msg.transaction_id, transaction_id);
    assert_eq!(msg.message_length, 0);
    assert_eq!(msg.attributes.len(), 0);
}

#[test]
fn test_message_encode_decode_roundtrip() {
    let transaction_id = [
        0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0, 0x11, 0x22, 0x33, 0x44,
    ];
    let msg = StunMessage::new(StunMessageType::Request, transaction_id);

    // Encode and decode without attributes
    let encoded = msg.encode();
    assert_eq!(encoded.len(), 20); // Header only

    let decoded = StunMessage::decode(&encoded).unwrap();
    assert_eq!(decoded.message_type, msg.message_type);
    assert_eq!(decoded.magic_cookie, msg.magic_cookie);
    assert_eq!(decoded.transaction_id, msg.transaction_id);
    assert_eq!(decoded.attributes.len(), 0);
}

#[test]
fn test_message_with_attribute() {
    let transaction_id = [0; 12];
    let mut msg = StunMessage::new(StunMessageType::Response, transaction_id);

    // Add a SOFTWARE attribute
    let software = b"Test STUN Server".to_vec();
    let attr = StunAttribute::new(StunAttributeType::Software, software.clone());
    msg.add_attribute(attr);

    // Encode
    let encoded = msg.encode();
    assert!(encoded.len() > 20);

    // Decode
    let decoded = StunMessage::decode(&encoded).unwrap();
    assert_eq!(decoded.attributes.len(), 1);
    assert_eq!(
        decoded.attributes[0].attribute_type,
        StunAttributeType::Software
    );
    assert_eq!(decoded.attributes[0].attribute_value, software);
}

#[test]
fn test_invalid_magic_cookie() {
    let mut data = vec![0u8; 20];
    data[0] = 0x00; // Request type
    data[1] = 0x01;
    data[2] = 0x00; // Length
    data[3] = 0x00;
    data[4] = 0xFF; // Invalid magic cookie
    data[5] = 0xFF;
    data[6] = 0xFF;
    data[7] = 0xFF;

    let result = StunMessage::decode(&data);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        StunError::MalformedMessage(_)
    ));
}

#[test]
fn test_trailing_bytes_rejected() {
    // RFC 5389 section 6: the message length delimits the message, so a
    // datagram carrying bytes after that boundary is malformed.
    let mut bytes = StunMessage::new(StunMessageType::Request, [0x42; 12]).encode();
    bytes.extend_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);

    let result = StunMessage::decode(&bytes);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        StunError::MalformedMessage(_)
    ));
}

#[test]
fn test_message_too_short() {
    let data = vec![0u8; 10]; // Too short for header
    let result = StunMessage::decode(&data);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        StunError::MalformedMessage(_)
    ));
}

#[test]
fn test_attribute_encode_decode() {
    let value = b"test value".to_vec();
    let attr = StunAttribute::new(StunAttributeType::Software, value.clone());

    let encoded = attr.encode();
    let decoded = StunAttribute::decode(&encoded).unwrap();

    assert_eq!(decoded.attribute_type, attr.attribute_type);
    assert_eq!(decoded.attribute_length, attr.attribute_length);
    assert_eq!(decoded.attribute_value, value);
}

#[test]
fn test_unknown_attribute_preserves_raw_type() {
    // Attribute with type 0xABCD (not defined by RFC 5389) and a 4-byte value.
    let raw = [0xAB, 0xCD, 0x00, 0x04, 0xDE, 0xAD, 0xBE, 0xEF];
    let attr = StunAttribute::decode(&raw).unwrap();

    assert_eq!(attr.attribute_type, StunAttributeType::Unknown);
    assert_eq!(attr.raw_type, 0xABCD);

    // Re-encoding must keep the original wire type instead of 0xFFFF.
    let encoded = attr.encode();
    assert_eq!(&encoded[0..2], &[0xAB, 0xCD]);
}

#[test]
fn test_known_attribute_raw_type_matches_enum() {
    let attr = StunAttribute::new(StunAttributeType::Software, b"x".to_vec());
    assert_eq!(attr.raw_type, 0x8022);
}

#[test]
fn test_attribute_padding() {
    // Test 5-byte value (needs 3 bytes padding)
    let value = vec![1, 2, 3, 4, 5];
    let attr = StunAttribute::new(StunAttributeType::Software, value);

    let encoded = attr.encode();
    // 4 bytes header + 5 bytes value + 3 bytes padding = 12 bytes
    assert_eq!(encoded.len(), 12);
    assert_eq!(attr.encoded_size(), 12);

    // Verify padding bytes are zero
    assert_eq!(encoded[9], 0);
    assert_eq!(encoded[10], 0);
    assert_eq!(encoded[11], 0);
}

#[test]
fn test_mapped_address_ipv4() {
    use std::net::Ipv4Addr;

    let addr = MappedAddress::new(std::net::IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)), 12345);

    assert_eq!(addr.family, AddressFamily::IPv4);
    assert_eq!(addr.port, 12345);

    let encoded = addr.encode();
    let decoded = MappedAddress::decode(&encoded).unwrap();

    assert_eq!(decoded.family, addr.family);
    assert_eq!(decoded.port, addr.port);
    assert_eq!(decoded.address, addr.address);
}

#[test]
fn test_mapped_address_ipv6() {
    use std::net::Ipv6Addr;

    let addr = MappedAddress::new(
        std::net::IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1)),
        54321,
    );

    assert_eq!(addr.family, AddressFamily::IPv6);
    assert_eq!(addr.port, 54321);

    let encoded = addr.encode();
    let decoded = MappedAddress::decode(&encoded).unwrap();

    assert_eq!(decoded.family, addr.family);
    assert_eq!(decoded.port, addr.port);
    assert_eq!(decoded.address, addr.address);
}

#[test]
fn test_decode_mapped_address_rejects_ipv4_trailing_bytes() {
    use std::net::{IpAddr, Ipv4Addr};

    let mut encoded = MappedAddress::new(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1)), 3478).encode();
    assert_eq!(encoded.len(), 8);
    encoded.push(0x00);

    assert!(
        MappedAddress::decode(&encoded).is_err(),
        "RFC 5389 section 15.1 fixes an IPv4 MAPPED-ADDRESS at 8 bytes"
    );
}

#[test]
fn test_decode_mapped_address_rejects_ipv6_trailing_bytes() {
    use std::net::{IpAddr, Ipv6Addr};

    let mut encoded = MappedAddress::new(
        IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1)),
        3478,
    )
    .encode();
    assert_eq!(encoded.len(), 20);
    encoded.push(0x00);

    assert!(
        MappedAddress::decode(&encoded).is_err(),
        "RFC 5389 section 15.1 fixes an IPv6 MAPPED-ADDRESS at 20 bytes"
    );
}

#[test]
fn test_xor_mapped_address_ipv4() {
    use std::net::Ipv4Addr;

    let addr = MappedAddress::new(std::net::IpAddr::V4(Ipv4Addr::new(203, 0, 113, 50)), 12345);

    let magic_cookie = 0x2112A442;
    let transaction_id = [
        0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0, 0x11, 0x22, 0x33, 0x44,
    ];

    let encoded = addr.encode_xor(magic_cookie, &transaction_id);
    let decoded = MappedAddress::decode_xor(&encoded, magic_cookie, &transaction_id).unwrap();

    assert_eq!(decoded.address, addr.address);
    assert_eq!(decoded.port, addr.port);
}

#[test]
fn test_decode_xor_exact_ipv4_length() {
    let magic_cookie = 0x2112A442;
    let transaction_id = [0u8; 12];

    let mut value = vec![0x00, 0x01, 0x00, 0x00];
    value.extend_from_slice(&[0u8; 4]);
    assert_eq!(value.len(), 8);

    assert!(MappedAddress::decode_xor(&value, magic_cookie, &transaction_id).is_ok());
}

#[test]
fn test_decode_xor_exact_ipv6_length() {
    let magic_cookie = 0x2112A442;
    let transaction_id = [0u8; 12];

    let mut value = vec![0x00, 0x02, 0x00, 0x00];
    value.extend_from_slice(&[0u8; 16]);
    assert_eq!(value.len(), 20);

    assert!(MappedAddress::decode_xor(&value, magic_cookie, &transaction_id).is_ok());
}

#[test]
fn test_decode_xor_ipv4_with_twenty_bytes_rejected() {
    let magic_cookie = 0x2112A442;
    let transaction_id = [0u8; 12];

    let mut value = vec![0x00, 0x01, 0x00, 0x00];
    value.extend_from_slice(&[0u8; 16]);

    let err = MappedAddress::decode_xor(&value, magic_cookie, &transaction_id).unwrap_err();
    assert!(matches!(err, StunError::MalformedMessage(_)));
}

#[test]
fn test_decode_xor_ipv6_with_eight_bytes_rejected() {
    let magic_cookie = 0x2112A442;
    let transaction_id = [0u8; 12];

    let mut value = vec![0x00, 0x02, 0x00, 0x00];
    value.extend_from_slice(&[0u8; 4]);

    let err = MappedAddress::decode_xor(&value, magic_cookie, &transaction_id).unwrap_err();
    assert!(matches!(err, StunError::MalformedMessage(_)));
}

#[test]
fn test_decode_xor_invalid_family_rejected() {
    let magic_cookie = 0x2112A442;
    let transaction_id = [0u8; 12];

    let mut value = vec![0x00, 0x03, 0x00, 0x00];
    value.extend_from_slice(&[0u8; 4]);

    let err = MappedAddress::decode_xor(&value, magic_cookie, &transaction_id).unwrap_err();
    assert!(matches!(err, StunError::MalformedMessage(_)));
}

#[test]
fn test_error_types() {
    let err1 = StunError::InvalidMessageType(0x9999);
    assert!(format!("{}", err1).contains("Invalid message type"));

    let err2 = StunError::MissingAttribute("MAPPED-ADDRESS".to_string());
    assert!(format!("{}", err2).contains("Missing required attribute"));

    let err3 = StunError::MalformedMessage("test error".to_string());
    assert!(format!("{}", err3).contains("Malformed message"));
}

#[test]
fn test_address_family_conversion() {
    assert_eq!(AddressFamily::from_raw(0x01).unwrap(), AddressFamily::IPv4);
    assert_eq!(AddressFamily::from_raw(0x02).unwrap(), AddressFamily::IPv6);
    assert!(AddressFamily::from_raw(0x03).is_err());
}

/// Encode a raw attribute (header + value + zero padding).
fn raw_attribute_bytes(raw_type: u16, value: &[u8]) -> Vec<u8> {
    let padding = (4 - value.len() % 4) % 4;
    let mut bytes = Vec::with_capacity(4 + value.len() + padding);
    bytes.extend_from_slice(&raw_type.to_be_bytes());
    bytes.extend_from_slice(&(value.len() as u16).to_be_bytes());
    bytes.extend_from_slice(value);
    bytes.resize(bytes.len() + padding, 0);
    bytes
}

/// Build a full Binding Request message carrying one raw attribute.
fn message_with_raw_attribute(raw_type: u16, value: &[u8]) -> Vec<u8> {
    let attribute = raw_attribute_bytes(raw_type, value);
    let mut bytes = Vec::with_capacity(20 + attribute.len());
    bytes.extend_from_slice(&0x0001u16.to_be_bytes());
    bytes.extend_from_slice(&(attribute.len() as u16).to_be_bytes());
    bytes.extend_from_slice(&0x2112A442u32.to_be_bytes());
    bytes.extend_from_slice(&[0x11; 12]);
    bytes.extend_from_slice(&attribute);
    bytes
}

#[test]
fn test_message_integrity_accepts_twenty_byte_value() {
    let attr = StunAttribute::decode(&raw_attribute_bytes(0x0008, &[0u8; 20])).unwrap();
    assert_eq!(attr.raw_type, 0x0008);
    assert_eq!(attr.attribute_length, 20);
    assert_eq!(attr.attribute_value.len(), 20);
}

#[test]
fn test_message_integrity_attribute_preserves_other_lengths() {
    // RFC 5389 section 15.4 defines a 20-byte value, but the structural decoder
    // must not reject other lengths: agents ignore attributes after the first
    // MESSAGE-INTEGRITY instead of discarding the whole message.
    for len in [0usize, 4, 16, 19, 21, 24] {
        let attr = StunAttribute::decode(&raw_attribute_bytes(0x0008, &vec![0u8; len]))
            .unwrap_or_else(|e| panic!("MESSAGE-INTEGRITY length {len} must decode: {e}"));
        assert_eq!(attr.attribute_length, len as u16);
        assert_eq!(attr.attribute_value.len(), len);
    }
}

#[test]
fn test_message_integrity_verification_rejects_other_lengths() {
    use stun_protocol::integrity::short_term_key;

    let key = short_term_key("VOkJxbRl1RmTxUk/WvJxBt").unwrap();
    for len in [0usize, 4, 16, 19, 21, 24] {
        let bytes = message_with_raw_attribute(0x0008, &vec![0u8; len]);
        assert!(
            !StunMessage::verify_message_integrity(&bytes, &key),
            "MESSAGE-INTEGRITY length {len} must not verify"
        );
    }
}

#[test]
fn test_truncated_duplicate_integrity_is_ignored() {
    use stun_protocol::integrity::short_term_key;

    // RFC 5389 section 15.4: everything after the first MESSAGE-INTEGRITY is
    // ignored, including a duplicate whose value is not 20 bytes.
    let key = short_term_key("VOkJxbRl1RmTxUk/WvJxBt").unwrap();
    let mut message = StunMessage::new(StunMessageType::Request, [0x11; 12]);
    message.add_attribute(StunAttribute::new(
        StunAttributeType::Software,
        b"test".to_vec(),
    ));
    let mut bytes = message.encode_with_integrity(&key);

    let duplicate = raw_attribute_bytes(0x0008, &[0xAA; 16]);
    let length = u16::from_be_bytes([bytes[2], bytes[3]]) + duplicate.len() as u16;
    bytes[2..4].copy_from_slice(&length.to_be_bytes());
    bytes.extend_from_slice(&duplicate);

    let decoded = StunMessage::decode(&bytes).expect("duplicate must be ignored, not rejected");
    assert_eq!(decoded.attributes.len(), 3);
    assert!(decoded.attributes[2].raw_type == 0x0008);
    assert_eq!(decoded.attributes[2].attribute_value.len(), 16);
    assert!(StunMessage::verify_message_integrity(&bytes, &key));
}

#[test]
fn test_fingerprint_accepts_four_byte_value() {
    let attr = StunAttribute::decode(&raw_attribute_bytes(0x8028, &[0u8; 4])).unwrap();
    assert_eq!(attr.raw_type, 0x8028);
    assert_eq!(attr.attribute_length, 4);
}

#[test]
fn test_fingerprint_attribute_preserves_other_lengths() {
    // RFC 5389 section 15.5 fixes a 4-byte value, but the structural decoder
    // keeps other lengths so FINGERPRINT verification can reject the message.
    for len in [0usize, 2, 5, 8] {
        let attr = StunAttribute::decode(&raw_attribute_bytes(0x8028, &vec![0u8; len]))
            .unwrap_or_else(|e| panic!("FINGERPRINT length {len} must decode: {e}"));
        assert_eq!(attr.attribute_length, len as u16);
        assert_eq!(attr.attribute_value.len(), len);
    }
}

#[test]
fn test_fingerprint_verification_rejects_other_lengths() {
    for len in [0usize, 2, 5, 8] {
        let bytes = message_with_raw_attribute(0x8028, &vec![0u8; len]);
        assert!(
            !StunMessage::verify_message_fingerprint(&bytes),
            "FINGERPRINT length {len} must not verify"
        );
    }
}

#[test]
fn test_attribute_offsets_accumulate_encoded_size() {
    let mut msg = StunMessage::new(StunMessageType::Request, [0x11; 12]);
    msg.add_attribute(StunAttribute::new(
        StunAttributeType::Software,
        b"test".to_vec(),
    ));
    msg.add_attribute(StunAttribute::new(
        StunAttributeType::Username,
        b"alice".to_vec(),
    ));

    // SOFTWARE(4+4=8), USERNAME(4+5+3 padding=12).
    assert_eq!(msg.attribute_offsets(), vec![20, 28]);
    assert_eq!(msg.attribute_count(0x8022), 1);
    assert_eq!(msg.attribute_count(0x0008), 0);
}

#[test]
fn test_encode_with_integrity_length_and_hmac_range() {
    use stun_protocol::integrity::{hmac_sha1, verify_hmac_sha1};

    let key = b"integrity-key";
    let mut msg = StunMessage::new(StunMessageType::Request, [0x22; 12]);
    msg.add_attribute(StunAttribute::new(
        StunAttributeType::Software,
        b"test-client".to_vec(),
    ));

    let bytes = msg.encode_with_integrity(key);

    // SOFTWARE encodes to 16 bytes, MESSAGE-INTEGRITY to 24 bytes.
    assert_eq!(bytes.len(), 20 + 16 + 24);
    assert_eq!(bytes.len(), 60);
    // The header length covers the MESSAGE-INTEGRITY attribute.
    assert_eq!(u16::from_be_bytes([bytes[2], bytes[3]]), 40);

    let mi_offset = 36;
    assert_eq!(&bytes[mi_offset..mi_offset + 2], &[0x00, 0x08]);
    assert_eq!(&bytes[mi_offset + 2..mi_offset + 4], &[0x00, 0x14]);

    // The digest is computed over the header (length adjusted to end at the
    // MESSAGE-INTEGRITY attribute) plus the preceding attributes only.
    let mut input = bytes[..mi_offset].to_vec();
    input[2..4].copy_from_slice(&40u16.to_be_bytes());
    let expected = hmac_sha1(key, &input);
    assert_eq!(&bytes[mi_offset + 4..], &expected[..]);
    assert!(verify_hmac_sha1(key, &input, &bytes[mi_offset + 4..]));
}

#[test]
fn test_encode_with_integrity_and_fingerprint_uses_mi_length_for_hmac() {
    use stun_protocol::integrity::{fingerprint, hmac_sha1};

    let key = b"integrity-key";
    let mut msg = StunMessage::new(StunMessageType::Response, [0x33; 12]);
    msg.add_attribute(StunAttribute::new(
        StunAttributeType::Software,
        b"test".to_vec(),
    ));

    let bytes = msg.encode_with_integrity_and_fingerprint(key);

    // 20-byte header + 8-byte SOFTWARE + 24-byte MI + 8-byte FINGERPRINT.
    assert_eq!(bytes.len(), 60);
    // The wire header length counts both trailing attributes.
    assert_eq!(u16::from_be_bytes([bytes[2], bytes[3]]), 40);

    let mi_offset = 28;
    let fp_offset = 52;
    assert_eq!(&bytes[fp_offset..fp_offset + 2], &[0x80, 0x28]);

    // HMAC input ends at MESSAGE-INTEGRITY, even though FINGERPRINT follows.
    let mut hmac_input = bytes[..mi_offset].to_vec();
    hmac_input[2..4].copy_from_slice(&32u16.to_be_bytes());
    assert_eq!(
        &bytes[mi_offset + 4..mi_offset + 24],
        &hmac_sha1(key, &hmac_input)
    );

    // FINGERPRINT is computed over the message with the final header length.
    assert_eq!(
        u32::from_be_bytes([
            bytes[fp_offset + 4],
            bytes[fp_offset + 5],
            bytes[fp_offset + 6],
            bytes[fp_offset + 7],
        ]),
        fingerprint(&bytes[..fp_offset])
    );
}

#[test]
fn test_encode_with_fingerprint_is_terminal() {
    use stun_protocol::integrity::verify_fingerprint;

    let mut msg = StunMessage::new(StunMessageType::Request, [0x44; 12]);
    msg.add_attribute(StunAttribute::new(
        StunAttributeType::Software,
        b"bench".to_vec(),
    ));

    let bytes = msg.encode_with_fingerprint();
    let fp_offset = bytes.len() - 8;

    assert_eq!(&bytes[fp_offset..fp_offset + 2], &[0x80, 0x28]);
    assert_eq!(
        u16::from_be_bytes([bytes[fp_offset + 2], bytes[fp_offset + 3]]),
        4
    );
    assert!(verify_fingerprint(
        &bytes[..fp_offset],
        &bytes[fp_offset + 4..]
    ));
}

#[test]
fn test_duplicate_integrity_attributes_are_countable() {
    let mut msg = StunMessage::new(StunMessageType::Request, [0x55; 12]);
    msg.add_attribute(StunAttribute::new(
        StunAttributeType::MessageIntegrity,
        vec![0u8; 20],
    ));
    msg.add_attribute(StunAttribute::new(
        StunAttributeType::MessageIntegrity,
        vec![0u8; 20],
    ));
    msg.add_attribute(StunAttribute::new(
        StunAttributeType::Fingerprint,
        vec![0u8; 4],
    ));
    msg.add_attribute(StunAttribute::new(
        StunAttributeType::Fingerprint,
        vec![0u8; 4],
    ));

    assert_eq!(msg.attribute_count(0x0008), 2);
    assert_eq!(msg.attribute_count(0x8028), 2);
}

#[test]
fn test_message_decode_preserves_attribute_lengths() {
    let valid = message_with_raw_attribute(0x0008, &[0u8; 20]);
    let decoded = StunMessage::decode(&valid).unwrap();
    assert_eq!(decoded.attributes[0].raw_type, 0x0008);

    // MESSAGE-INTEGRITY length is enforced when the digest is verified, so a
    // malformed duplicate after the first one is ignored (RFC 5389 section
    // 15.4) rather than rejecting the whole message.
    let short_integrity = message_with_raw_attribute(0x0008, &[0u8; 16]);
    let decoded = StunMessage::decode(&short_integrity)
        .expect("a short MESSAGE-INTEGRITY is structurally valid");
    assert_eq!(decoded.attributes[0].attribute_value.len(), 16);
    assert!(!StunMessage::verify_message_integrity(
        &short_integrity,
        b"any-key"
    ));

    // FINGERPRINT length is enforced when the CRC is verified (RFC 5389
    // section 15.5), not while parsing attributes.
    let invalid_fingerprint = message_with_raw_attribute(0x8028, &[0u8; 8]);
    let decoded = StunMessage::decode(&invalid_fingerprint)
        .expect("an oversized FINGERPRINT is structurally valid");
    assert_eq!(decoded.attributes[0].attribute_value.len(), 8);
    assert!(!StunMessage::verify_message_fingerprint(
        &invalid_fingerprint
    ));
}

/// Build a Binding Request from raw attribute encodings with a header length
/// covering every supplied attribute.
fn request_with_raw_attributes(attributes: &[Vec<u8>]) -> Vec<u8> {
    let total: usize = attributes.iter().map(|attribute| attribute.len()).sum();
    let mut bytes = Vec::with_capacity(20 + total);
    bytes.extend_from_slice(&0x0001u16.to_be_bytes());
    bytes.extend_from_slice(&(total as u16).to_be_bytes());
    bytes.extend_from_slice(&0x2112A442u32.to_be_bytes());
    bytes.extend_from_slice(&[0x77; 12]);
    for attribute in attributes {
        bytes.extend_from_slice(attribute);
    }
    bytes
}

#[test]
fn test_verify_fingerprint_uses_final_length_header() {
    use stun_protocol::integrity::{fingerprint, verify_fingerprint};

    let mut msg = StunMessage::new(StunMessageType::Request, [0x66; 12]);
    msg.add_attribute(StunAttribute::new(
        StunAttributeType::Software,
        b"fp".to_vec(),
    ));
    let bytes = msg.encode_with_fingerprint();
    let fp_offset = bytes.len() - 8;

    // The header length already counts the FINGERPRINT attribute itself.
    assert_eq!(
        usize::from(u16::from_be_bytes([bytes[2], bytes[3]])),
        bytes.len() - 20
    );
    assert_eq!(
        u32::from_be_bytes([
            bytes[fp_offset + 4],
            bytes[fp_offset + 5],
            bytes[fp_offset + 6],
            bytes[fp_offset + 7],
        ]),
        fingerprint(&bytes[..fp_offset])
    );
    assert!(verify_fingerprint(
        &bytes[..fp_offset],
        &bytes[fp_offset + 4..]
    ));
    assert!(StunMessage::verify_message_fingerprint(&bytes));
}

#[test]
fn test_verify_fingerprint_rejects_non_final_attribute() {
    use stun_protocol::integrity::fingerprint;

    let mut bytes = request_with_raw_attributes(&[
        raw_attribute_bytes(0x8028, &[0u8; 4]),
        raw_attribute_bytes(0x8022, b"tail"),
    ]);
    // Give the FINGERPRINT a correct CRC for a prefix that ends before it even
    // though it is not the final attribute.
    let crc = fingerprint(&bytes[..20]);
    bytes[24..28].copy_from_slice(&crc.to_be_bytes());

    assert!(
        !StunMessage::verify_message_fingerprint(&bytes),
        "FINGERPRINT must be the final attribute"
    );
}

#[test]
fn test_verify_fingerprint_rejects_duplicate_attributes() {
    use stun_protocol::integrity::fingerprint;

    let mut bytes = request_with_raw_attributes(&[
        raw_attribute_bytes(0x8028, &[0u8; 4]),
        raw_attribute_bytes(0x8028, &[0u8; 4]),
    ]);
    // The first FINGERPRINT carries a correct CRC so only uniqueness rejects it.
    let crc = fingerprint(&bytes[..20]);
    bytes[24..28].copy_from_slice(&crc.to_be_bytes());

    assert!(
        !StunMessage::verify_message_fingerprint(&bytes),
        "duplicate FINGERPRINT must be rejected"
    );
}

#[test]
fn test_verify_integrity_ignores_duplicate_attributes() {
    use stun_protocol::integrity::{hmac_input, hmac_sha1};

    let key = b"integrity-key";
    let mut bytes = request_with_raw_attributes(&[
        raw_attribute_bytes(0x0008, &[0u8; 20]),
        raw_attribute_bytes(0x0008, &[0u8; 20]),
    ]);
    // The first MESSAGE-INTEGRITY is valid; the second is ignored because
    // RFC 5389 section 15.4 ignores attributes after the first digest.
    let digest = hmac_sha1(key, &hmac_input(&bytes, 20));
    bytes[24..44].copy_from_slice(&digest);

    assert!(StunMessage::verify_message_integrity(&bytes, key));
    assert!(!StunMessage::verify_message_integrity(&bytes, b"wrong-key"));
}

#[test]
fn test_verify_integrity_accepts_single_valid_attribute() {
    let key = b"integrity-key";
    let mut msg = StunMessage::new(StunMessageType::Request, [0x77; 12]);
    msg.add_attribute(StunAttribute::new(
        StunAttributeType::Software,
        b"client".to_vec(),
    ));
    let bytes = msg.encode_with_integrity(key);

    assert!(StunMessage::verify_message_integrity(&bytes, key));
}
