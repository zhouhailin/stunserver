//! Integration tests for STUN Client.
//!
//! Tests cover request building, response parsing, timeout handling,
//! and error cases by running a mock STUN server alongside the client.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use stun_protocol::{
    MappedAddress, StunAttribute, StunAttributeType, StunMessage, StunMessageType,
};
use stunclient::client;
use tokio::net::UdpSocket;

/// Spawn a mock STUN server that responds to binding requests.
/// Returns the server's listen address.
async fn spawn_mock_server(respond_with: MockResponse) -> SocketAddr {
    let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let addr = socket.local_addr().unwrap();
    let socket = Arc::new(socket);

    tokio::spawn(async move {
        let mut buf = vec![0u8; 65536];
        // Handle up to 10 requests then exit.
        for _ in 0..10 {
            let (len, from) = match socket.recv_from(&mut buf).await {
                Ok(r) => r,
                Err(_) => break,
            };

            let response_bytes = match respond_with {
                MockResponse::Valid => {
                    let request = StunMessage::decode(&buf[..len]).unwrap();
                    let mut response =
                        StunMessage::new(StunMessageType::Response, request.transaction_id);

                    // Echo back the client's address as the mapped address.
                    let mapped = MappedAddress::new(from.ip(), from.port());
                    let xor_value =
                        mapped.encode_xor(response.magic_cookie, &response.transaction_id);
                    let attr = StunAttribute::new(StunAttributeType::XorMappedAddress, xor_value);
                    response.add_attribute(attr);
                    response.encode()
                }
                MockResponse::NoAttribute => {
                    let request = StunMessage::decode(&buf[..len]).unwrap();
                    let response =
                        StunMessage::new(StunMessageType::Response, request.transaction_id);
                    response.encode()
                }
                MockResponse::Silent => {
                    // Don't respond at all.
                    continue;
                }
            };

            let _ = socket.send_to(&response_bytes, from).await;
        }
    });

    addr
}

enum MockResponse {
    Valid,
    NoAttribute,
    Silent,
}

#[tokio::test]
async fn test_binding_request_success() {
    let server_addr = spawn_mock_server(MockResponse::Valid).await;

    let result = client::send_binding_request(server_addr, Duration::from_secs(5))
        .await
        .unwrap();

    // The mapped address should be our local socket address.
    assert!(result.mapped_address.port() > 0);
    assert!(result.latency.as_millis() < 5000);
}

#[tokio::test]
async fn test_binding_request_timeout() {
    let server_addr = spawn_mock_server(MockResponse::Silent).await;

    let result = client::send_binding_request(server_addr, Duration::from_millis(100)).await;

    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        client::ClientError::Timeout(_)
    ));
}

#[tokio::test]
async fn test_binding_request_missing_attribute() {
    let server_addr = spawn_mock_server(MockResponse::NoAttribute).await;

    let result = client::send_binding_request(server_addr, Duration::from_secs(5)).await;

    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        client::ClientError::Protocol(_)
    ));
}

#[tokio::test]
async fn test_binding_request_unreachable_server() {
    // Use a non-routable address to simulate unreachable server.
    let unreachable: SocketAddr = "192.0.2.1:3478".parse().unwrap();

    let result = client::send_binding_request(unreachable, Duration::from_millis(200)).await;

    assert!(result.is_err());
    // Should be either timeout or connection error.
    match result.unwrap_err() {
        client::ClientError::Timeout(_) | client::ClientError::Connection(_) => {}
        other => panic!("unexpected error: {:?}", other),
    }
}

#[tokio::test]
async fn test_binding_request_ipv6() {
    // Bind a mock server on IPv6 loopback.
    let socket = UdpSocket::bind("[::1]:0").await.unwrap();
    let server_addr = socket.local_addr().unwrap();
    let socket = Arc::new(socket);

    tokio::spawn(async move {
        let mut buf = vec![0u8; 65536];
        let (len, from) = socket.recv_from(&mut buf).await.unwrap();
        let request = StunMessage::decode(&buf[..len]).unwrap();
        let mut response = StunMessage::new(StunMessageType::Response, request.transaction_id);
        let mapped = MappedAddress::new(from.ip(), from.port());
        let xor_value = mapped.encode_xor(response.magic_cookie, &response.transaction_id);
        let attr = StunAttribute::new(StunAttributeType::XorMappedAddress, xor_value);
        response.add_attribute(attr);
        let _ = socket.send_to(&response.encode(), from).await;
    });

    let result = client::send_binding_request(server_addr, Duration::from_secs(5))
        .await
        .unwrap();

    assert!(result.mapped_address.ip().is_loopback());
}
