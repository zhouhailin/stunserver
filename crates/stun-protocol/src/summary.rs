//! Human-readable single-line summaries of STUN messages for logging.

use crate::{MappedAddress, StunAttribute, StunAttributeType, StunMessage, StunMessageType};
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
        StunAttributeType::XorMappedAddress => {
            match MappedAddress::decode_xor(&attr.attribute_value, magic_cookie, transaction_id) {
                Ok(address) => format!("{name}={}", format_address(address)),
                Err(_) => format!("{name}(len={length})"),
            }
        }
        StunAttributeType::MappedAddress
        | StunAttributeType::SourceAddress
        | StunAttributeType::ChangedAddress
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
        StunAttributeType::ErrorCode => "ERROR-CODE",
        StunAttributeType::XorMappedAddress => "XOR-MAPPED-ADDRESS",
        StunAttributeType::Software => "SOFTWARE",
        StunAttributeType::SourceAddress => "SOURCE-ADDRESS",
        StunAttributeType::ChangedAddress => "CHANGED-ADDRESS",
        StunAttributeType::OtherAddress => "OTHER-ADDRESS",
        StunAttributeType::Unknown => "UNKNOWN",
    }
}

/// Format a decoded MappedAddress the same way a SocketAddr is displayed.
fn format_address(address: MappedAddress) -> String {
    SocketAddr::new(address.address, address.port).to_string()
}
