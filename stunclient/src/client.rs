//! STUN Client implementation.
//!
//! Sends STUN Binding Requests to a server and parses the response
//! to extract the client's reflexive (mapped) address.

use std::net::SocketAddr;
use std::time::{Duration, Instant};
use stun_protocol::{MappedAddress, StunAttributeType, StunMessage, StunMessageType};
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

/// Parse a STUN Binding Response and extract the mapped address.
fn parse_response(
    data: &[u8],
    expected_transaction_id: &[u8; 12],
    latency: Duration,
) -> Result<BindingResponse, ClientError> {
    let response = StunMessage::decode(data)
        .map_err(|e| ClientError::Protocol(format!("failed to decode response: {}", e)))?;

    // Verify message type is a success response.
    if response.message_type != StunMessageType::Response {
        return Err(ClientError::Protocol(format!(
            "unexpected message type: {:?}",
            response.message_type
        )));
    }

    // Verify transaction ID matches.
    if response.transaction_id != *expected_transaction_id {
        return Err(ClientError::Protocol("transaction ID mismatch".to_string()));
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
}
