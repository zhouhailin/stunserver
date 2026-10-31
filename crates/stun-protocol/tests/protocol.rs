//! Unit tests for STUN protocol library

use stun_protocol::*;

#[test]
fn test_message_type_conversion() {
    assert_eq!(StunMessageType::Request.to_raw(), 0x0001);
    assert_eq!(StunMessageType::Response.to_raw(), 0x0101);
    assert_eq!(StunMessageType::ErrorIndication.to_raw(), 0x0111);

    assert_eq!(
        StunMessageType::from_raw(0x0001).unwrap(),
        StunMessageType::Request
    );
    assert_eq!(
        StunMessageType::from_raw(0x0101).unwrap(),
        StunMessageType::Response
    );
    assert_eq!(
        StunMessageType::from_raw(0x0111).unwrap(),
        StunMessageType::ErrorIndication
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
