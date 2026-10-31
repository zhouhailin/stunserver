//! Human-readable single-line summaries of STUN messages for logging.

use crate::{
    ErrorCode, MappedAddress, StunAttribute, StunAttributeType, StunMessage, StunMessageType,
    UnknownAttributes,
};
use std::net::SocketAddr;

/// Format a decoded STUN message as a single-line DEBUG summary.
pub fn describe_message(message: &StunMessage) -> String {
    let transaction_id: String = message
        .transaction_id
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect();
    let attributes: Vec<String> = message
        .attributes
        .iter()
        .map(|attr| describe_attribute(attr, message.magic_cookie, &message.transaction_id))
        .collect();

    format!(
        "{} txn={} attrs=[{}]",
        message_type_name(message.message_type),
        transaction_id,
        attributes.join(", ")
    )
}

/// Format a single attribute for the DEBUG message summary.
pub fn describe_attribute(
    attr: &StunAttribute,
    magic_cookie: u32,
    transaction_id: &[u8; 12],
) -> String {
    let length = attr.attribute_length;
    let name = attribute_name(attr.attribute_type);

    match attr.attribute_type {
        StunAttributeType::Software => {
            format!(
                "SOFTWARE=\"{}\"",
                String::from_utf8_lossy(&attr.attribute_value)
            )
        }
        StunAttributeType::ErrorCode => match ErrorCode::decode_value(&attr.attribute_value) {
            Ok(code) => format!("ERROR-CODE={} \"{}\"", code.status_code(), code.reason),
            Err(_) => format!("{name}(len={length})"),
        },
        StunAttributeType::UnknownAttributes => {
            match UnknownAttributes::decode_value(&attr.attribute_value) {
                Ok(list) => {
                    let types: Vec<String> =
                        list.types.iter().map(|t| format!("0x{:04X}", t)).collect();
                    format!("{name}=[{}]", types.join(", "))
                }
                Err(_) => format!("{name}(len={length})"),
            }
        }
        StunAttributeType::XorMappedAddress => {
            match MappedAddress::decode_xor(&attr.attribute_value, magic_cookie, transaction_id) {
                Ok(address) => format!("{name}={}", format_address(address)),
                Err(_) => format!("{name}(len={length})"),
            }
        }
        StunAttributeType::MappedAddress
        | StunAttributeType::ResponseOrigin
        | StunAttributeType::OtherAddress => match MappedAddress::decode(&attr.attribute_value) {
            Ok(address) => format!("{name}={}", format_address(address)),
            Err(_) => format!("{name}(len={length})"),
        },
        StunAttributeType::Unknown => format!("0x{:04X}(len={length})", attr.raw_type),
        _ => format!("{name}(len={length})"),
    }
}

/// Human-readable name for a STUN message type.
fn message_type_name(message_type: StunMessageType) -> &'static str {
    match message_type {
        StunMessageType::Request => "Binding Request",
        StunMessageType::Response => "Binding Success Response",
        StunMessageType::ErrorIndication => "Binding Error Response",
    }
}

/// Human-readable name for a known STUN attribute type.
fn attribute_name(attribute_type: StunAttributeType) -> &'static str {
    match attribute_type {
        StunAttributeType::MappedAddress => "MAPPED-ADDRESS",
        StunAttributeType::ChangeRequest => "CHANGE-REQUEST",
        StunAttributeType::ErrorCode => "ERROR-CODE",
        StunAttributeType::UnknownAttributes => "UNKNOWN-ATTRIBUTES",
        StunAttributeType::XorMappedAddress => "XOR-MAPPED-ADDRESS",
        StunAttributeType::Padding => "PADDING",
        StunAttributeType::ResponsePort => "RESPONSE-PORT",
        StunAttributeType::Software => "SOFTWARE",
        StunAttributeType::CacheTimeout => "CACHE-TIMEOUT",
        StunAttributeType::ResponseOrigin => "RESPONSE-ORIGIN",
        StunAttributeType::OtherAddress => "OTHER-ADDRESS",
        StunAttributeType::Unknown => "UNKNOWN",
    }
}

/// Format a decoded MappedAddress the same way a SocketAddr is displayed.
fn format_address(address: MappedAddress) -> String {
    SocketAddr::new(address.address, address.port).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ErrorCode, UnknownAttributes};

    const MAGIC_COOKIE: u32 = 0x2112A442;
    const TXN: [u8; 12] = [0xAB; 12];

    #[test]
    fn test_describe_rfc5780_names() {
        assert_eq!(
            describe_attribute(
                &StunAttribute::new(
                    StunAttributeType::ChangeRequest,
                    vec![0x00, 0x00, 0x00, 0x06]
                ),
                MAGIC_COOKIE,
                &TXN
            ),
            "CHANGE-REQUEST(len=4)"
        );
        assert_eq!(
            describe_attribute(
                &StunAttribute::new(StunAttributeType::Padding, vec![0u8; 4]),
                MAGIC_COOKIE,
                &TXN
            ),
            "PADDING(len=4)"
        );
        assert_eq!(
            describe_attribute(
                &StunAttribute::new(StunAttributeType::ResponsePort, vec![0x1F, 0x90]),
                MAGIC_COOKIE,
                &TXN
            ),
            "RESPONSE-PORT(len=2)"
        );
        assert_eq!(
            describe_attribute(
                &StunAttribute::new(
                    StunAttributeType::CacheTimeout,
                    vec![0x00, 0x00, 0x00, 0x1E]
                ),
                MAGIC_COOKIE,
                &TXN
            ),
            "CACHE-TIMEOUT(len=4)"
        );
    }

    #[test]
    fn test_describe_response_origin_and_other_address() {
        use crate::AddressFamily;
        use std::net::{IpAddr, Ipv4Addr};

        let address = MappedAddress {
            family: AddressFamily::IPv4,
            port: 3478,
            address: IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1)),
        };
        let value = address.encode();

        assert_eq!(
            describe_attribute(
                &StunAttribute::new(StunAttributeType::ResponseOrigin, value.clone()),
                MAGIC_COOKIE,
                &TXN
            ),
            "RESPONSE-ORIGIN=192.0.2.1:3478"
        );
        assert_eq!(
            describe_attribute(
                &StunAttribute::new(StunAttributeType::OtherAddress, value),
                MAGIC_COOKIE,
                &TXN
            ),
            "OTHER-ADDRESS=192.0.2.1:3478"
        );
    }

    #[test]
    fn test_describe_error_code() {
        let code = ErrorCode::new(4, 20, "Unknown Attribute".to_string());
        let attr = StunAttribute::new(StunAttributeType::ErrorCode, code.encode_value());

        assert_eq!(
            describe_attribute(&attr, MAGIC_COOKIE, &TXN),
            "ERROR-CODE=420 \"Unknown Attribute\""
        );
    }

    #[test]
    fn test_describe_unknown_attributes() {
        let list = UnknownAttributes::new(vec![0x0024, 0x0025]);
        let attr = StunAttribute::new(StunAttributeType::UnknownAttributes, list.encode_value());

        assert_eq!(
            describe_attribute(&attr, MAGIC_COOKIE, &TXN),
            "UNKNOWN-ATTRIBUTES=[0x0024, 0x0025]"
        );
    }

    #[test]
    fn test_describe_error_code_decode_failure() {
        let attr = StunAttribute::new(StunAttributeType::ErrorCode, vec![0x00, 0x00]);

        assert_eq!(
            describe_attribute(&attr, MAGIC_COOKIE, &TXN),
            "ERROR-CODE(len=2)"
        );
    }

    #[test]
    fn test_describe_unknown_attributes_decode_failure() {
        let attr = StunAttribute::new(StunAttributeType::UnknownAttributes, vec![0x00, 0x24, 0x00]);

        assert_eq!(
            describe_attribute(&attr, MAGIC_COOKIE, &TXN),
            "UNKNOWN-ATTRIBUTES(len=3)"
        );
    }

    #[test]
    fn test_describe_error_response_message() {
        let mut message = StunMessage::new(StunMessageType::ErrorIndication, TXN);
        let code = ErrorCode::new(4, 20, "Unknown Attribute".to_string());
        message.add_attribute(StunAttribute::new(
            StunAttributeType::ErrorCode,
            code.encode_value(),
        ));
        message.add_attribute(StunAttribute::new(
            StunAttributeType::UnknownAttributes,
            UnknownAttributes::new(vec![0x0024]).encode_value(),
        ));

        assert_eq!(
            describe_message(&message),
            format!(
                "Binding Error Response txn={} attrs=[ERROR-CODE=420 \"Unknown Attribute\", UNKNOWN-ATTRIBUTES=[0x0024]]",
                TXN.iter().map(|b| format!("{:02x}", b)).collect::<String>()
            )
        );
    }
}
