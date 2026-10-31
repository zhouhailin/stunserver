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

/// RFC 5769 section 2.1 sample Binding Request (108 bytes).
const SAMPLE_REQUEST: [u8; 108] = [
    0x00, 0x01, 0x00, 0x58, 0x21, 0x12, 0xa4, 0x42, 0xb7, 0xe7, 0xa7, 0x01, 0xbc, 0x34, 0xd6, 0x86,
    0xfa, 0x87, 0xdf, 0xae, 0x80, 0x22, 0x00, 0x10, 0x53, 0x54, 0x55, 0x4e, 0x20, 0x74, 0x65, 0x73,
    0x74, 0x20, 0x63, 0x6c, 0x69, 0x65, 0x6e, 0x74, 0x00, 0x24, 0x00, 0x04, 0x6e, 0x00, 0x01, 0xff,
    0x80, 0x29, 0x00, 0x08, 0x93, 0x2f, 0xf9, 0xb1, 0x51, 0x26, 0x3b, 0x36, 0x00, 0x06, 0x00, 0x09,
    0x65, 0x76, 0x74, 0x6a, 0x3a, 0x68, 0x36, 0x76, 0x59, 0x20, 0x20, 0x20, 0x00, 0x08, 0x00, 0x14,
    0x9a, 0xea, 0xa7, 0x0c, 0xbf, 0xd8, 0xcb, 0x56, 0x78, 0x1e, 0xf2, 0xb5, 0xb2, 0xd3, 0xf2, 0x49,
    0xc1, 0xb5, 0x71, 0xa2, 0x80, 0x28, 0x00, 0x04, 0xe5, 0x7a, 0x3b, 0xcf,
];

/// RFC 5769 section 2.2 sample IPv4 Binding Success Response (80 bytes).
const SAMPLE_IPV4_RESPONSE: [u8; 80] = [
    0x01, 0x01, 0x00, 0x3c, 0x21, 0x12, 0xa4, 0x42, 0xb7, 0xe7, 0xa7, 0x01, 0xbc, 0x34, 0xd6, 0x86,
    0xfa, 0x87, 0xdf, 0xae, 0x80, 0x22, 0x00, 0x0b, 0x74, 0x65, 0x73, 0x74, 0x20, 0x76, 0x65, 0x63,
    0x74, 0x6f, 0x72, 0x20, 0x00, 0x20, 0x00, 0x08, 0x00, 0x01, 0xa1, 0x47, 0xe1, 0x12, 0xa6, 0x43,
    0x00, 0x08, 0x00, 0x14, 0x2b, 0x91, 0xf5, 0x99, 0xfd, 0x9e, 0x90, 0xc3, 0x8c, 0x74, 0x89, 0xf9,
    0x2a, 0xf9, 0xba, 0x53, 0xf0, 0x6b, 0xe7, 0xd7, 0x80, 0x28, 0x00, 0x04, 0xc0, 0x7d, 0x4c, 0x96,
];

/// RFC 5769 section 2.3 sample IPv6 Binding Success Response (92 bytes).
const SAMPLE_IPV6_RESPONSE: [u8; 92] = [
    0x01, 0x01, 0x00, 0x48, 0x21, 0x12, 0xa4, 0x42, 0xb7, 0xe7, 0xa7, 0x01, 0xbc, 0x34, 0xd6, 0x86,
    0xfa, 0x87, 0xdf, 0xae, 0x80, 0x22, 0x00, 0x0b, 0x74, 0x65, 0x73, 0x74, 0x20, 0x76, 0x65, 0x63,
    0x74, 0x6f, 0x72, 0x20, 0x00, 0x20, 0x00, 0x14, 0x00, 0x02, 0xa1, 0x47, 0x01, 0x13, 0xa9, 0xfa,
    0xa5, 0xd3, 0xf1, 0x79, 0xbc, 0x25, 0xf4, 0xb5, 0xbe, 0xd2, 0xb9, 0xd9, 0x00, 0x08, 0x00, 0x14,
    0xa3, 0x82, 0x95, 0x4e, 0x4b, 0xe6, 0x7b, 0xf1, 0x17, 0x84, 0xc9, 0x7c, 0x82, 0x92, 0xc2, 0x75,
    0xbf, 0xe3, 0xed, 0x41, 0x80, 0x28, 0x00, 0x04, 0xc8, 0xfb, 0x0b, 0x4c,
];

/// RFC 5769 section 2.4 sample request with long-term authentication (116 bytes).
const SAMPLE_LONG_TERM_REQUEST: [u8; 116] = [
    0x00, 0x01, 0x00, 0x60, 0x21, 0x12, 0xa4, 0x42, 0x78, 0xad, 0x34, 0x33, 0xc6, 0xad, 0x72, 0xc0,
    0x29, 0xda, 0x41, 0x2e, 0x00, 0x06, 0x00, 0x12, 0xe3, 0x83, 0x9e, 0xe3, 0x83, 0x88, 0xe3, 0x83,
    0xaa, 0xe3, 0x83, 0x83, 0xe3, 0x82, 0xaf, 0xe3, 0x82, 0xb9, 0x00, 0x00, 0x00, 0x15, 0x00, 0x1c,
    0x66, 0x2f, 0x2f, 0x34, 0x39, 0x39, 0x6b, 0x39, 0x35, 0x34, 0x64, 0x36, 0x4f, 0x4c, 0x33, 0x34,
    0x6f, 0x4c, 0x39, 0x46, 0x53, 0x54, 0x76, 0x79, 0x36, 0x34, 0x73, 0x41, 0x00, 0x14, 0x00, 0x0b,
    0x65, 0x78, 0x61, 0x6d, 0x70, 0x6c, 0x65, 0x2e, 0x6f, 0x72, 0x67, 0x00, 0x00, 0x08, 0x00, 0x14,
    0xf6, 0x70, 0x24, 0x65, 0x6d, 0xd6, 0x4a, 0x3e, 0x02, 0xb8, 0xe0, 0x71, 0x2e, 0x85, 0xc9, 0xa2,
    0x8c, 0xa8, 0x96, 0x66,
];

/// RFC 5769 section 2.1: exact request bytes decode to documented values and
/// re-encode byte-for-byte, including the space (0x20) USERNAME padding.
#[test]
fn test_rfc5769_sample_request() {
    let message = StunMessage::decode(&SAMPLE_REQUEST).expect("decode sample request");

    assert_eq!(message.message_type, StunMessageType::Request);
    assert_eq!(message.message_length, 0x58);
    assert_eq!(message.magic_cookie, 0x2112A442);
    assert_eq!(
        message.transaction_id,
        [0xb7, 0xe7, 0xa7, 0x01, 0xbc, 0x34, 0xd6, 0x86, 0xfa, 0x87, 0xdf, 0xae]
    );
    assert_eq!(message.attributes.len(), 6);

    assert_eq!(message.attributes[0].raw_type, 0x8022);
    assert_eq!(message.attributes[0].attribute_value, b"STUN test client");
    assert_eq!(message.attributes[1].raw_type, 0x0024);
    assert_eq!(
        message.attributes[1].attribute_value,
        [0x6e, 0x00, 0x01, 0xff]
    );
    assert_eq!(message.attributes[2].raw_type, 0x8029);
    assert_eq!(message.attributes[2].attribute_value.len(), 8);
    assert_eq!(message.attributes[3].raw_type, 0x0006);
    assert_eq!(message.attributes[3].attribute_value, b"evtj:h6vY");
    assert_eq!(message.attributes[3].padding, [0x20, 0x20, 0x20]);
    assert_eq!(message.attributes[4].raw_type, 0x0008);
    assert_eq!(message.attributes[4].attribute_value.len(), 20);
    assert_eq!(message.attributes[5].raw_type, 0x8028);
    assert_eq!(message.attributes[5].attribute_value.len(), 4);

    assert_eq!(message.encode(), SAMPLE_REQUEST);
}

/// RFC 5769 section 2.2: IPv4 response decodes to 192.0.2.1:32853.
#[test]
fn test_rfc5769_sample_ipv4_response() {
    let message = StunMessage::decode(&SAMPLE_IPV4_RESPONSE).expect("decode IPv4 response");

    assert_eq!(message.message_type, StunMessageType::Response);
    assert_eq!(message.message_length, 0x3c);
    assert_eq!(message.attributes.len(), 4);
    assert_eq!(message.attributes[0].raw_type, 0x8022);
    assert_eq!(message.attributes[0].attribute_value, b"test vector");
    assert_eq!(message.attributes[1].raw_type, 0x0020);

    let mapped = MappedAddress::decode_xor(
        &message.attributes[1].attribute_value,
        message.magic_cookie,
        &message.transaction_id,
    )
    .expect("decode XOR-MAPPED-ADDRESS");
    assert_eq!(mapped.port, 32853);
    assert_eq!(
        mapped.address,
        std::net::IpAddr::V4(std::net::Ipv4Addr::new(192, 0, 2, 1))
    );

    assert_eq!(message.encode(), SAMPLE_IPV4_RESPONSE);
}

/// RFC 5769 section 2.3: IPv6 response decodes to the documented address.
#[test]
fn test_rfc5769_sample_ipv6_response() {
    let message = StunMessage::decode(&SAMPLE_IPV6_RESPONSE).expect("decode IPv6 response");

    assert_eq!(message.message_type, StunMessageType::Response);
    assert_eq!(message.message_length, 0x48);
    assert_eq!(message.attributes.len(), 4);
    assert_eq!(message.attributes[1].raw_type, 0x0020);

    let mapped = MappedAddress::decode_xor(
        &message.attributes[1].attribute_value,
        message.magic_cookie,
        &message.transaction_id,
    )
    .expect("decode XOR-MAPPED-ADDRESS");
    assert_eq!(mapped.port, 32853);
    assert_eq!(
        mapped.address,
        std::net::IpAddr::V6(std::net::Ipv6Addr::new(
            0x2001, 0x0db8, 0x1234, 0x5678, 0x0011, 0x2233, 0x4455, 0x6677
        ))
    );

    assert_eq!(message.encode(), SAMPLE_IPV6_RESPONSE);
}

/// RFC 5769 section 2.4: long-term credential request decodes and re-encodes
/// byte-for-byte, preserving USERNAME and REALM zero padding.
#[test]
fn test_rfc5769_sample_long_term_request() {
    let message = StunMessage::decode(&SAMPLE_LONG_TERM_REQUEST).expect("decode long-term request");

    assert_eq!(message.message_type, StunMessageType::Request);
    assert_eq!(message.message_length, 0x60);
    assert_eq!(message.attributes.len(), 4);

    assert_eq!(message.attributes[0].raw_type, 0x0006);
    assert_eq!(
        String::from_utf8(message.attributes[0].attribute_value.clone()).unwrap(),
        "\u{30de}\u{30c8}\u{30ea}\u{30c3}\u{30af}\u{30b9}"
    );
    assert_eq!(message.attributes[0].padding, [0x00, 0x00]);

    assert_eq!(message.attributes[1].raw_type, 0x0015);
    assert_eq!(
        message.attributes[1].attribute_value,
        b"f//499k954d6OL34oL9FSTvy64sA"
    );

    assert_eq!(message.attributes[2].raw_type, 0x0014);
    assert_eq!(message.attributes[2].attribute_value, b"example.org");
    assert_eq!(message.attributes[2].padding, [0x00]);

    assert_eq!(message.attributes[3].raw_type, 0x0008);
    assert_eq!(message.attributes[3].attribute_value.len(), 20);

    assert_eq!(message.encode(), SAMPLE_LONG_TERM_REQUEST);
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

/// Test that non-zero attribute padding survives a decode/encode round trip.
///
/// RFC 5769 section 2.1 pads the 9-byte USERNAME with three space characters.
/// The padding value is not significant on the wire, but a byte-identical
/// re-encode must preserve it (RFC 5389 section 15: padding "MAY be any value").
#[test]
fn test_username_space_padding_preserved() {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&0x0001u16.to_be_bytes()); // Binding Request
    bytes.extend_from_slice(&0x0010u16.to_be_bytes()); // 16 attribute bytes
    bytes.extend_from_slice(&0x2112A442u32.to_be_bytes());
    bytes.extend_from_slice(&[
        0xb7, 0xe7, 0xa7, 0x01, 0xbc, 0x34, 0xd6, 0x86, 0xfa, 0x87, 0xdf, 0xae,
    ]);
    bytes.extend_from_slice(&0x0006u16.to_be_bytes()); // USERNAME
    bytes.extend_from_slice(&0x0009u16.to_be_bytes()); // 9-byte value
    bytes.extend_from_slice(b"test-user");
    bytes.extend_from_slice(&[0x20, 0x20, 0x20]); // space padding

    let message = StunMessage::decode(&bytes).expect("decode message");
    assert_eq!(message.attributes.len(), 1);
    assert_eq!(message.attributes[0].attribute_value, b"test-user");

    let reencoded = message.encode();
    assert_eq!(reencoded, bytes, "re-encode must preserve wire padding");
    assert_eq!(&reencoded[33..36], &[0x20, 0x20, 0x20]);
}

/// Newly constructed attributes still emit zero padding.
#[test]
fn test_new_attribute_emits_zero_padding() {
    let attr = StunAttribute::new(StunAttributeType::Software, vec![b'a'; 9]);
    let encoded = attr.encode();

    assert_eq!(encoded.len(), 16);
    assert_eq!(&encoded[13..16], &[0x00, 0x00, 0x00]);
}

/// An attribute whose value runs to the end with padding missing is rejected.
#[test]
fn test_attribute_truncated_padding_rejected() {
    let mut raw = vec![0x00, 0x06, 0x00, 0x09];
    raw.extend_from_slice(b"test-user");

    let result = StunAttribute::decode(&raw);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        StunError::MalformedMessage(_)
    ));
}

/// A message claiming padded length but ending before the padding is rejected.
#[test]
fn test_message_truncated_padding_rejected() {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&0x0001u16.to_be_bytes());
    bytes.extend_from_slice(&0x0010u16.to_be_bytes()); // claims 16 attribute bytes
    bytes.extend_from_slice(&0x2112A442u32.to_be_bytes());
    bytes.extend_from_slice(&[0u8; 12]);
    bytes.extend_from_slice(&0x0006u16.to_be_bytes());
    bytes.extend_from_slice(&0x0009u16.to_be_bytes());
    bytes.extend_from_slice(b"test-user"); // 9 value bytes, no padding

    assert!(StunMessage::decode(&bytes).is_err());
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
