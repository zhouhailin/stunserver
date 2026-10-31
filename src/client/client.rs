//! STUN Client implementation.
//!
//! Sends STUN Binding Requests to a server and parses the response
//! to extract the client's reflexive (mapped) address.

use std::net::SocketAddr;
use std::time::{Duration, Instant};
use stun_protocol::{
    describe_message, is_comprehension_required, ErrorCode, MappedAddress, StunAttributeType,
    StunMessage, StunMessageType, UnknownAttributes,
};
use tokio::net::UdpSocket;

/// Result of a successful STUN binding request.
#[derive(Debug, Clone)]
pub struct BindingResponse {
    /// The mapped (reflexive) address as seen by the STUN server.
    pub mapped_address: SocketAddr,
    /// Round-trip latency of the request.
    pub latency: Duration,
}

/// Errors that can occur during a STUN binding request.
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("Connection timeout after {0} seconds")]
    Timeout(u64),

    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Invalid server address: {0}")]
    InvalidAddress(String),

    #[error("{}", format_server_error(class, number, reason, unknown_attributes))]
    ServerError {
        class: u8,
        number: u8,
        reason: String,
        unknown_attributes: Vec<u16>,
    },
}

fn format_server_error(class: &u8, number: &u8, reason: &str, unknown: &[u16]) -> String {
    let status = u16::from(*class) * 100 + u16::from(*number);
    let list = if unknown.is_empty() {
        "none".to_string()
    } else {
        unknown
            .iter()
            .map(|t| format!("0x{:04X}", t))
            .collect::<Vec<_>>()
            .join(", ")
    };
    format!("server returned error {status} ({reason}); unknown attributes: {list}")
}

/// Send a STUN Binding Request to the specified server address.
///
/// Creates a UDP socket, sends a Binding Request with a random transaction ID,
/// waits for a response (up to `timeout` seconds), and extracts the
/// XOR-MAPPED-ADDRESS from the response.
///
/// Returns the client's reflexive address and the round-trip latency.
pub async fn send_binding_request(
    server_addr: SocketAddr,
    timeout: Duration,
) -> Result<BindingResponse, ClientError> {
    // Bind to any available local port.
    let local_addr: SocketAddr = if server_addr.is_ipv4() {
        "0.0.0.0:0".parse().unwrap()
    } else {
        "[::]:0".parse().unwrap()
    };

    let socket = UdpSocket::bind(local_addr)
        .await
        .map_err(|e| ClientError::Connection(format!("failed to bind socket: {}", e)))?;

    // Build STUN Binding Request with random transaction ID.
    let transaction_id = generate_transaction_id();
    let request = StunMessage::new(StunMessageType::Request, transaction_id);
    let request_bytes = request.encode();

    log::debug!("Sending: {}", describe_message(&request));
    log::trace!("Sending: {:02x?}", request_bytes);

    let start = Instant::now();

    // Send request.
    socket
        .send_to(&request_bytes, server_addr)
        .await
        .map_err(|e| ClientError::Connection(format!("failed to send request: {}", e)))?;

    // Wait for response with timeout.
    let mut recv_buf = vec![0u8; 65536];
    let recv_result = tokio::time::timeout(timeout, socket.recv_from(&mut recv_buf)).await;

    let latency = start.elapsed();

    match recv_result {
        Ok(Ok((len, from_addr))) => {
            log::debug!("Received {} bytes from {}", len, from_addr);
            log::trace!("Received: {:02x?}", &recv_buf[..len]);

            // Verify response came from the expected server.
            if from_addr != server_addr {
                return Err(ClientError::Protocol(format!(
                    "response from unexpected address: {} (expected {})",
                    from_addr, server_addr
                )));
            }

            parse_response(&recv_buf[..len], &transaction_id, latency)
        }
        Ok(Err(e)) => Err(ClientError::Connection(format!("recv error: {}", e))),
        Err(_) => Err(ClientError::Timeout(timeout.as_secs())),
    }
}

/// Send a STUN Binding Request with fallback to multiple addresses.
///
/// Tries each address in order until one succeeds. Returns the first successful
/// response or an error if all addresses fail.
pub async fn send_binding_request_with_fallback(
    server_addrs: &[SocketAddr],
    timeout: Duration,
) -> Result<BindingResponse, ClientError> {
    if server_addrs.is_empty() {
        return Err(ClientError::InvalidAddress(
            "no addresses provided".to_string(),
        ));
    }

    let mut last_error = None;

    for addr in server_addrs {
        match send_binding_request(*addr, timeout).await {
            Ok(response) => return Ok(response),
            Err(e) => {
                log::debug!("Failed to connect to {}: {}", addr, e);
                last_error = Some(e);
            }
        }
    }

    Err(last_error.unwrap_or_else(|| ClientError::Connection("all addresses failed".to_string())))
}

/// Parse a STUN Binding Response and extract the mapped address.
fn parse_response(
    data: &[u8],
    expected_transaction_id: &[u8; 12],
    latency: Duration,
) -> Result<BindingResponse, ClientError> {
    let response = StunMessage::decode(data).map_err(|e| {
        log::debug!("Failed to decode STUN message ({} bytes)", data.len());
        ClientError::Protocol(format!("failed to decode response: {}", e))
    })?;

    log::debug!("Received: {}", describe_message(&response));

    // Verify transaction ID matches before interpreting the message.
    if response.transaction_id != *expected_transaction_id {
        return Err(ClientError::Protocol("transaction ID mismatch".to_string()));
    }

    match response.message_type {
        StunMessageType::ErrorIndication => return Err(parse_server_error(&response)),
        StunMessageType::Response => {}
        StunMessageType::Request => {
            return Err(ClientError::Protocol(format!(
                "unexpected message type: {:?}",
                response.message_type
            )));
        }
    }

    // A success response carrying a comprehension-required attribute the
    // client does not recognize cannot be trusted (RFC 5389 section 7.3.1).
    if let Some(attr) = response
        .attributes
        .iter()
        .find(|a| is_comprehension_required(a.raw_type) && !client_recognizes(a.raw_type))
    {
        return Err(ClientError::Protocol(format!(
            "success response contains unknown required attribute 0x{:04X}",
            attr.raw_type
        )));
    }

    // Find XOR-MAPPED-ADDRESS attribute.
    let xor_attr = response
        .attributes
        .iter()
        .find(|a| a.attribute_type == StunAttributeType::XorMappedAddress)
        .ok_or_else(|| ClientError::Protocol("response missing XOR-MAPPED-ADDRESS".to_string()))?;

    // Decode the mapped address.
    let mapped = MappedAddress::decode_xor(
        &xor_attr.attribute_value,
        response.magic_cookie,
        &response.transaction_id,
    )
    .map_err(|e| ClientError::Protocol(format!("failed to decode mapped address: {}", e)))?;

    let mapped_address = SocketAddr::new(mapped.address, mapped.port);

    Ok(BindingResponse {
        mapped_address,
        latency,
    })
}

/// Comprehension-required attributes this client understands in a response.
fn client_recognizes(raw_type: u16) -> bool {
    matches!(raw_type, 0x0001 | 0x0009 | 0x000A | 0x0020)
}

/// Convert a decoded 0x0111 response into a structured client error.
fn parse_server_error(response: &StunMessage) -> ClientError {
    let code = response
        .attributes
        .iter()
        .find(|a| a.attribute_type == StunAttributeType::ErrorCode)
        .and_then(|a| ErrorCode::decode_value(&a.attribute_value).ok());

    let Some(code) = code else {
        return ClientError::Protocol("error response missing valid ERROR-CODE".to_string());
    };

    let unknown_attributes = response
        .attributes
        .iter()
        .find(|a| a.attribute_type == StunAttributeType::UnknownAttributes)
        .and_then(|a| UnknownAttributes::decode_value(&a.attribute_value).ok())
        .map(|list| list.types)
        .unwrap_or_default();

    ClientError::ServerError {
        class: code.class,
        number: code.number,
        reason: code.reason,
        unknown_attributes,
    }
}

/// Generate a random 12-byte transaction ID.
fn generate_transaction_id() -> [u8; 12] {
    let mut id = [0u8; 12];
    // Use a simple counter + timestamp for uniqueness in tests.
    // In production, this should use a cryptographic RNG.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let bytes = now.to_le_bytes();
    for i in 0..12 {
        id[i] = bytes[i % bytes.len()];
    }
    id
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_transaction_id() {
        let id1 = generate_transaction_id();
        let id2 = generate_transaction_id();
        // IDs should be different (unless called in the same nanosecond, extremely unlikely).
        assert_ne!(id1, id2);
        assert_eq!(id1.len(), 12);
    }

    #[test]
    fn test_parse_response_valid() {
        // Build a valid response.
        let transaction_id = [0x42; 12];
        let mut response = StunMessage::new(StunMessageType::Response, transaction_id);

        // Add XOR-MAPPED-ADDRESS for 192.168.1.100:5000.
        let mapped = MappedAddress::new("192.168.1.100".parse().unwrap(), 5000);
        let xor_value = mapped.encode_xor(response.magic_cookie, &transaction_id);
        let attr =
            stun_protocol::StunAttribute::new(StunAttributeType::XorMappedAddress, xor_value);
        response.add_attribute(attr);

        let response_bytes = response.encode();
        let latency = Duration::from_millis(5);

        let result = parse_response(&response_bytes, &transaction_id, latency).unwrap();
        assert_eq!(
            result.mapped_address,
            "192.168.1.100:5000".parse::<SocketAddr>().unwrap()
        );
        assert_eq!(result.latency, latency);
    }

    #[test]
    fn test_parse_response_wrong_transaction_id() {
        let transaction_id = [0x42; 12];
        let response = StunMessage::new(StunMessageType::Response, transaction_id);
        let response_bytes = response.encode();

        let wrong_id = [0x99; 12];
        let result = parse_response(&response_bytes, &wrong_id, Duration::from_millis(1));
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ClientError::Protocol(_)));
    }

    #[test]
    fn test_parse_response_wrong_message_type() {
        let transaction_id = [0x42; 12];
        let request = StunMessage::new(StunMessageType::Request, transaction_id);
        let request_bytes = request.encode();

        let result = parse_response(&request_bytes, &transaction_id, Duration::from_millis(1));
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ClientError::Protocol(_)));
    }

    #[test]
    fn test_parse_response_missing_attribute() {
        let transaction_id = [0x42; 12];
        let response = StunMessage::new(StunMessageType::Response, transaction_id);
        // No attributes added.
        let response_bytes = response.encode();

        let result = parse_response(&response_bytes, &transaction_id, Duration::from_millis(1));
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ClientError::Protocol(_)));
    }

    #[test]
    fn test_parse_response_invalid_data() {
        let transaction_id = [0x42; 12];
        let result = parse_response(&[0, 1, 2], &transaction_id, Duration::from_millis(1));
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ClientError::Protocol(_)));
    }

    fn raw_attribute(raw_type: u16, value: &[u8]) -> stun_protocol::StunAttribute {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&raw_type.to_be_bytes());
        bytes.extend_from_slice(&(value.len() as u16).to_be_bytes());
        bytes.extend_from_slice(value);
        stun_protocol::StunAttribute::decode(&bytes).unwrap()
    }

    fn error_response(transaction_id: [u8; 12], unknown: Option<Vec<u16>>) -> Vec<u8> {
        use stun_protocol::{ErrorCode, UnknownAttributes};

        let mut response = StunMessage::new(StunMessageType::ErrorIndication, transaction_id);
        let code = ErrorCode::new(4, 20, "Unknown Attribute".to_string());
        response.add_attribute(stun_protocol::StunAttribute::new(
            StunAttributeType::ErrorCode,
            code.encode_value(),
        ));
        if let Some(types) = unknown {
            response.add_attribute(stun_protocol::StunAttribute::new(
                StunAttributeType::UnknownAttributes,
                UnknownAttributes::new(types).encode_value(),
            ));
        }
        response.encode()
    }

    #[test]
    fn test_parse_response_420_with_unknown_attributes() {
        let transaction_id = [0x42; 12];
        let bytes = error_response(transaction_id, Some(vec![0x0024]));

        let err = parse_response(&bytes, &transaction_id, Duration::from_millis(1)).unwrap_err();

        match err {
            ClientError::ServerError {
                class,
                number,
                reason,
                unknown_attributes,
            } => {
                assert_eq!(class, 4);
                assert_eq!(number, 20);
                assert_eq!(reason, "Unknown Attribute");
                assert_eq!(unknown_attributes, vec![0x0024]);
            }
            other => panic!("unexpected error: {:?}", other),
        }
    }

    #[test]
    fn test_parse_response_420_without_unknown_attributes() {
        let transaction_id = [0x42; 12];
        let bytes = error_response(transaction_id, None);

        let err = parse_response(&bytes, &transaction_id, Duration::from_millis(1)).unwrap_err();

        match err {
            ClientError::ServerError {
                class,
                number,
                unknown_attributes,
                ..
            } => {
                assert_eq!(class, 4);
                assert_eq!(number, 20);
                assert!(unknown_attributes.is_empty());
            }
            other => panic!("unexpected error: {:?}", other),
        }
    }

    #[test]
    fn test_parse_response_error_without_error_code() {
        let transaction_id = [0x42; 12];
        let response = StunMessage::new(StunMessageType::ErrorIndication, transaction_id);

        let result = parse_response(
            &response.encode(),
            &transaction_id,
            Duration::from_millis(1),
        );
        assert!(matches!(result.unwrap_err(), ClientError::Protocol(_)));
    }

    #[test]
    fn test_parse_response_error_with_wrong_transaction_id() {
        let bytes = error_response([0x42; 12], Some(vec![0x0024]));
        let wrong_id = [0x99; 12];

        let result = parse_response(&bytes, &wrong_id, Duration::from_millis(1));
        assert!(matches!(result.unwrap_err(), ClientError::Protocol(_)));
    }

    #[test]
    fn test_parse_response_success_with_unknown_optional_attribute() {
        let transaction_id = [0x42; 12];
        let mut response = StunMessage::new(StunMessageType::Response, transaction_id);
        let mapped = MappedAddress::new("192.0.2.1".parse().unwrap(), 32853);
        let xor_value = mapped.encode_xor(response.magic_cookie, &transaction_id);
        response.add_attribute(stun_protocol::StunAttribute::new(
            StunAttributeType::XorMappedAddress,
            xor_value,
        ));
        response.add_attribute(raw_attribute(0x80FF, &[0xDE, 0xAD, 0xBE, 0xEF]));

        let result = parse_response(
            &response.encode(),
            &transaction_id,
            Duration::from_millis(1),
        )
        .unwrap();
        assert_eq!(
            result.mapped_address,
            "192.0.2.1:32853".parse::<SocketAddr>().unwrap()
        );
    }

    #[test]
    fn test_parse_response_success_with_unknown_required_attribute() {
        let transaction_id = [0x42; 12];
        let mut response = StunMessage::new(StunMessageType::Response, transaction_id);
        let mapped = MappedAddress::new("192.0.2.1".parse().unwrap(), 32853);
        let xor_value = mapped.encode_xor(response.magic_cookie, &transaction_id);
        response.add_attribute(stun_protocol::StunAttribute::new(
            StunAttributeType::XorMappedAddress,
            xor_value,
        ));
        response.add_attribute(raw_attribute(0x0024, &[0x00, 0x00, 0x00, 0x00]));

        let result = parse_response(
            &response.encode(),
            &transaction_id,
            Duration::from_millis(1),
        );
        assert!(matches!(result.unwrap_err(), ClientError::Protocol(_)));
    }
}
