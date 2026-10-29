//! RFC 5769 Test Vectors for STUN Protocol Compliance
//!
//! This module implements test vectors inspired by RFC 5769 to verify STUN protocol
//! compliance. These test vectors ensure correct implementation of STUN message
//! parsing, serialization, and XOR-MAPPED-ADDRESS encoding/decoding.
//!
//! Reference: https://tools.ietf.org/html/rfc5769

use stun_protocol::*;

/// Test STUN message format compliance with RFC 5389 Section 6
///
/// Verifies that STUN messages follow the correct format:
/// - 20-byte header
/// - Magic cookie: 0x2112A442
/// - 12-byte transaction ID
/// - Message length field
/// - Attributes follow header
#[test]
fn test_rfc5389_message_format() {
    let transaction_id = [
        0xb7, 0xe7, 0xa7, 0x01, 0xbc, 0x34, 0xd6, 0x86, 0xfa, 0x87, 0xdf, 0xae,
    ];

    // Create a request message
    let message = StunMessage::new(StunMessageType::Request, transaction_id);

    // Encode
    let encoded = message.encode();

    // Verify header format (RFC 5389 Section 6)
    assert_eq!(encoded.len(), 20, "STUN header must be 20 bytes");

    // Verify message type (first 2 bytes)
    assert_eq!(encoded[0], 0x00);
    assert_eq!(encoded[1], 0x01, "Request type must be 0x0001");

    // Verify message length (bytes 2-3)
    assert_eq!(encoded[2], 0x00);
    assert_eq!(encoded[3], 0x00, "Length must be 0 for header-only message");

    // Verify magic cookie (bytes 4-7)
    assert_eq!(encoded[4], 0x21);
    assert_eq!(encoded[5], 0x12);
    assert_eq!(encoded[6], 0xA4);
    assert_eq!(encoded[7], 0x42, "Magic cookie must be 0x2112A442");

    // Verify transaction ID (bytes 8-19)
    assert_eq!(&encoded[8..20], &transaction_id);
}

/// Test XOR-MAPPED-ADDRESS encoding/decoding for IPv4
///
/// Verifies correct XOR encoding as specified in RFC 5389 Section 15.2
#[test]
fn test_rfc5769_xor_mapped_address_ipv4() {
    use std::net::{IpAddr, Ipv4Addr};

    // Test address from RFC 5769 examples
    let original_address = MappedAddress::new(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1)), 3478);

    // Use standard magic cookie and transaction ID
    let magic_cookie = 0x2112A442;
    let transaction_id = [
        0xb7, 0xe7, 0xa7, 0x01, 0xbc, 0x34, 0xd6, 0x86, 0xfa, 0x87, 0xdf, 0xae,
    ];

    // Encode XOR-MAPPED-ADDRESS
    let encoded = original_address.encode_xor(magic_cookie, &transaction_id);

    // Verify encoding produces correct length
    assert_eq!(encoded.len(), 8, "IPv4 XOR-MAPPED-ADDRESS must be 8 bytes");

    // Decode XOR-MAPPED-ADDRESS
    let decoded = MappedAddress::decode_xor(&encoded, magic_cookie, &transaction_id)
        .expect("Failed to decode XOR-MAPPED-ADDRESS");

    // Verify round-trip
    assert_eq!(decoded.address, original_address.address);
    assert_eq!(decoded.port, original_address.port);
    assert_eq!(decoded.family, AddressFamily::IPv4);
}

/// Test XOR-MAPPED-ADDRESS encoding/decoding for IPv6
///
/// Verifies correct XOR encoding for IPv6 addresses
#[test]
fn test_rfc5769_xor_mapped_address_ipv6() {
    use std::net::{IpAddr, Ipv6Addr};

    // Test IPv6 address
    let original_address = MappedAddress::new(
        IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1)),
        3478,
    );

    let magic_cookie = 0x2112A442;
    let transaction_id = [
        0xb7, 0xe7, 0xa7, 0x01, 0xbc, 0x34, 0xd6, 0x86, 0xfa, 0x87, 0xdf, 0xae,
    ];

    // Encode XOR-MAPPED-ADDRESS
    let encoded = original_address.encode_xor(magic_cookie, &transaction_id);

    // Verify encoding produces correct length
    assert_eq!(
        encoded.len(),
        20,
        "IPv6 XOR-MAPPED-ADDRESS must be 20 bytes"
    );

    // Decode XOR-MAPPED-ADDRESS
    let decoded = MappedAddress::decode_xor(&encoded, magic_cookie, &transaction_id)
        .expect("Failed to decode XOR-MAPPED-ADDRESS");

    // Verify round-trip
    assert_eq!(decoded.address, original_address.address);
    assert_eq!(decoded.port, original_address.port);
    assert_eq!(decoded.family, AddressFamily::IPv6);
}

/// Test STUN message with multiple attributes
///
/// Verifies correct encoding/decoding of messages with attributes
#[test]
fn test_rfc5769_message_with_attributes() {
    let transaction_id = [
        0xb7, 0xe7, 0xa7, 0x01, 0xbc, 0x34, 0xd6, 0x86, 0xfa, 0x87, 0xdf, 0xae,
    ];

    // Create a response message
    let mut message = StunMessage::new(StunMessageType::Response, transaction_id);

    // Add SOFTWARE attribute
    let software = b"RFC 5769 Test".to_vec();
    let attr1 = StunAttribute::new(StunAttributeType::Software, software.clone());
    message.add_attribute(attr1);

    // Add XOR-MAPPED-ADDRESS attribute
    use std::net::{IpAddr, Ipv4Addr};
    let mapped_addr = MappedAddress::new(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1)), 3478);
    let xor_encoded = mapped_addr.encode_xor(message.magic_cookie, &transaction_id);
    let attr2 = StunAttribute::new(StunAttributeType::XorMappedAddress, xor_encoded);
    message.add_attribute(attr2);

    // Encode
    let encoded = message.encode();

    // Verify message length is correct
    assert!(
        encoded.len() > 20,
        "Message with attributes must be > 20 bytes"
    );

    // Decode
    let decoded = StunMessage::decode(&encoded).expect("Failed to decode message");

    // Verify
    assert_eq!(decoded.message_type, StunMessageType::Response);
    assert_eq!(decoded.magic_cookie, 0x2112A442);
    assert_eq!(decoded.transaction_id, transaction_id);
    assert_eq!(decoded.attributes.len(), 2);

    // Verify first attribute
    assert_eq!(
        decoded.attributes[0].attribute_type,
        StunAttributeType::Software
    );
    assert_eq!(decoded.attributes[0].attribute_value, software);

    // Verify second attribute
    assert_eq!(
        decoded.attributes[1].attribute_type,
        StunAttributeType::XorMappedAddress
    );
}

/// Test magic cookie validation
///
/// Verifies that messages with invalid magic cookie are rejected per RFC 5389
#[test]
fn test_rfc5769_magic_cookie_validation() {
    let transaction_id = [0u8; 12];
    let message = StunMessage::new(StunMessageType::Request, transaction_id);
    let mut encoded = message.encode();

    // Corrupt the magic cookie
    encoded[4] = 0xFF;
    encoded[5] = 0xFF;
    encoded[6] = 0xFF;
    encoded[7] = 0xFF;

    // Should fail to decode
    let result = StunMessage::decode(&encoded);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        StunError::MalformedMessage(_)
    ));
}

/// Test message type validation
///
/// Verifies that only valid message types are accepted
#[test]
fn test_rfc5769_message_type_validation() {
    // Valid message types
    assert_eq!(StunMessageType::Request.to_raw(), 0x0001);
    assert_eq!(StunMessageType::Response.to_raw(), 0x0101);
    assert_eq!(StunMessageType::ErrorIndication.to_raw(), 0x0111);

    // Invalid message type
    let result = StunMessageType::from_raw(0x9999);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        StunError::InvalidMessageType(_)
    ));
}

/// Test attribute padding
///
/// Verifies that attributes are padded to 4-byte boundaries per RFC 5389
#[test]
fn test_rfc5769_attribute_padding() {
    // Test various attribute lengths
    for len in [1, 2, 3, 4, 5, 6, 7, 8] {
        let value = vec![0u8; len];
        let attr = StunAttribute::new(StunAttributeType::Software, value);

        let encoded = attr.encode();

        // Verify padding: total length must be multiple of 4
        let total_len = 4 + len; // 4-byte header + value
        let padded_len = (total_len + 3) & !3; // Round up to multiple of 4
        assert_eq!(
            encoded.len(),
            padded_len,
            "Attribute with {}-byte value must be padded to {} bytes",
            len,
            padded_len
        );

        // Verify padding bytes are zero
        for (i, byte) in encoded.iter().enumerate().take(padded_len).skip(4 + len) {
            assert_eq!(*byte, 0, "Padding byte at position {} must be zero", i);
        }
    }
}

/// Test STUN message round-trip encoding/decoding
///
/// Verifies that messages can be encoded and decoded without loss
#[test]
fn test_rfc5769_message_roundtrip() {
    let transaction_id = [
        0xb7, 0xe7, 0xa7, 0x01, 0xbc, 0x34, 0xd6, 0x86, 0xfa, 0x87, 0xdf, 0xae,
    ];

    // Create a request message
    let mut message = StunMessage::new(StunMessageType::Request, transaction_id);

    // Add SOFTWARE attribute
    let software = b"test vector".to_vec();
    let attr = StunAttribute::new(StunAttributeType::Software, software.clone());
    message.add_attribute(attr);

    // Encode
    let encoded = message.encode();

    // Decode
    let decoded = StunMessage::decode(&encoded).expect("Failed to decode message");

    // Verify
    assert_eq!(decoded.message_type, message.message_type);
    assert_eq!(decoded.magic_cookie, message.magic_cookie);
    assert_eq!(decoded.transaction_id, message.transaction_id);
    assert_eq!(decoded.attributes.len(), 1);
    assert_eq!(
        decoded.attributes[0].attribute_type,
        StunAttributeType::Software
    );
    assert_eq!(decoded.attributes[0].attribute_value, software);
}

/// Test address family encoding
///
/// Verifies correct encoding of IPv4 and IPv6 address families
#[test]
fn test_rfc5769_address_family_encoding() {
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    // IPv4
    let ipv4_addr = MappedAddress::new(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)), 12345);
    assert_eq!(ipv4_addr.family, AddressFamily::IPv4);
    let encoded = ipv4_addr.encode();
    let decoded = MappedAddress::decode(&encoded).expect("Failed to decode IPv4");
    assert_eq!(decoded.family, AddressFamily::IPv4);

    // IPv6
    let ipv6_addr = MappedAddress::new(
        IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1)),
        54321,
    );
    assert_eq!(ipv6_addr.family, AddressFamily::IPv6);
    let encoded = ipv6_addr.encode();
    let decoded = MappedAddress::decode(&encoded).expect("Failed to decode IPv6");
    assert_eq!(decoded.family, AddressFamily::IPv6);
}
