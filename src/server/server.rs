//! STUN Server implementation.
//!
//! High-performance async UDP server using Tokio. Handles STUN Binding
//! requests by responding with XOR-MAPPED-ADDRESS containing the client's
//! reflexive address.

use crate::config::ServerConfig;
use chrono::Utc;
use log::{debug, error, info, warn};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use stun_protocol::{
    MappedAddress, StunAttribute, StunAttributeType, StunMessage, StunMessageType,
};
use tokio::net::UdpSocket;
use tokio::signal;
use tokio::sync::Notify;

/// Size of the pre-allocated receive buffer (64 KB, well above STUN max).
const RECV_BUF_SIZE: usize = 65536;

/// Runtime server state shared across all request handler tasks.
pub struct ServerState {
    /// Server startup timestamp (UTC).
    pub start_time: chrono::DateTime<Utc>,
    /// System timezone description.
    pub timezone: String,
    /// Total requests processed.
    pub request_count: AtomicU64,
    /// Total errors encountered.
    pub error_count: AtomicU64,
}

impl ServerState {
    /// Create a new server state with counters initialized to zero.
    pub fn new() -> Self {
        let timezone = chrono::Local::now().offset().to_string();
        Self {
            start_time: Utc::now(),
            timezone,
            request_count: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
        }
    }
}

impl Default for ServerState {
    fn default() -> Self {
        Self::new()
    }
}

impl ServerState {
    /// Increment the request counter and return the new value.
    pub fn inc_requests(&self) -> u64 {
        self.request_count.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// Increment the error counter and return the new value.
    pub fn inc_errors(&self) -> u64 {
        self.error_count.fetch_add(1, Ordering::Relaxed) + 1
    }
}

/// Process a single STUN request and produce a response buffer.
///
/// Parses the incoming UDP payload as a STUN Binding Request, validates it,
/// and constructs a Success Response with XOR-MAPPED-ADDRESS reflecting
/// the client's source address.
///
/// Returns `None` if the request is invalid (not a STUN message, wrong type, etc.).
pub fn build_response(buf: &[u8], client_addr: SocketAddr) -> Option<Vec<u8>> {
    let request = StunMessage::decode(buf).ok()?;

    if request.message_type != StunMessageType::Request {
        return None;
    }

    // Build success response with same transaction ID.
    let mut response = StunMessage::new(StunMessageType::Response, request.transaction_id);

    // Create XOR-MAPPED-ADDRESS attribute with client's reflexive address.
    let mapped = MappedAddress::new(client_addr.ip(), client_addr.port());
    let xor_value = mapped.encode_xor(request.magic_cookie, &request.transaction_id);
    let attr = StunAttribute::new(StunAttributeType::XorMappedAddress, xor_value);
    response.add_attribute(attr);

    Some(response.encode())
}

/// Run the STUN server.
///
/// Binds to the configured listen address, receives UDP packets, processes
/// STUN Binding Requests, and sends responses. Runs until a SIGINT signal
/// is received.
pub async fn run_server(
    config: &ServerConfig,
    state: Arc<ServerState>,
) -> Result<(), Box<dyn std::error::Error>> {
    let listen_addr = config.listen;

    let socket = UdpSocket::bind(listen_addr).await?;
    let socket = Arc::new(socket);

    info!("STUN server listening on {}", listen_addr);

    // Shutdown coordination.
    let shutdown = Arc::new(Notify::new());
    let shutdown_signal = shutdown.clone();

    // Spawn signal handler task.
    tokio::spawn(async move {
        wait_for_shutdown().await;
        info!("Shutdown signal received, stopping server...");
        shutdown_signal.notify_one();
    });

    let mut buf = vec![0u8; RECV_BUF_SIZE];

    loop {
        tokio::select! {
            result = socket.recv_from(&mut buf) => {
                match result {
                    Ok((len, client_addr)) => {
                        let sock = socket.clone();
                        let state = state.clone();
                        let data = buf[..len].to_vec();

                        // Spawn a task for each request to maximize concurrency.
                        tokio::spawn(async move {
                            state.inc_requests();
                            debug!("Request from {} ({} bytes)", client_addr, len);
                            debug!("Received: {:02x?}", data);

                            match build_response(&data, client_addr) {
                                Some(response) => {
                                    debug!("Response: {:02x?}", response);
                                    if let Err(e) = sock.send_to(&response, client_addr).await {
                                        state.inc_errors();
                                        warn!("Failed to send response to {}: {}", client_addr, e);
                                    }
                                }
                                None => {
                                    state.inc_errors();
                                    debug!("Invalid STUN request from {}", client_addr);
                                }
                            }
                        });
                    }
                    Err(e) => {
                        state.inc_errors();
                        error!("Failed to receive UDP packet: {}", e);
                    }
                }
            }
            _ = shutdown.notified() => {
                info!(
                    "Server shutting down. Requests processed: {}, errors: {}",
                    state.request_count.load(Ordering::Relaxed),
                    state.error_count.load(Ordering::Relaxed)
                );
                break;
            }
        }
    }

    Ok(())
}

/// Wait for a SIGINT (Ctrl+C) signal.
async fn wait_for_shutdown() {
    if let Err(e) = signal::ctrl_c().await {
        error!("Failed to listen for shutdown signal: {}", e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stun_protocol::{StunMessage, StunMessageType};

    #[test]
    fn test_server_state_new() {
        let state = ServerState::new();
        assert_eq!(state.request_count.load(Ordering::Relaxed), 0);
        assert_eq!(state.error_count.load(Ordering::Relaxed), 0);
        assert!(!state.timezone.is_empty());
    }

    #[test]
    fn test_server_state_inc_requests() {
        let state = ServerState::new();
        assert_eq!(state.inc_requests(), 1);
        assert_eq!(state.inc_requests(), 2);
        assert_eq!(state.request_count.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn test_server_state_inc_errors() {
        let state = ServerState::new();
        assert_eq!(state.inc_errors(), 1);
        assert_eq!(state.inc_errors(), 2);
        assert_eq!(state.error_count.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn test_build_response_ipv4() {
        // Build a valid STUN Binding Request.
        let transaction_id = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
        let request = StunMessage::new(StunMessageType::Request, transaction_id);
        let request_bytes = request.encode();

        let client_addr: SocketAddr = "192.168.1.100:5000".parse().unwrap();
        let response_bytes = build_response(&request_bytes, client_addr).unwrap();

        // Decode the response and verify.
        let response = StunMessage::decode(&response_bytes).unwrap();
        assert_eq!(response.message_type, StunMessageType::Response);
        assert_eq!(response.transaction_id, transaction_id);
        assert_eq!(response.attributes.len(), 1);
        assert_eq!(
            response.attributes[0].attribute_type,
            StunAttributeType::XorMappedAddress
        );

        // Decode the XOR-MAPPED-ADDRESS and verify it matches client address.
        let mapped = MappedAddress::decode_xor(
            &response.attributes[0].attribute_value,
            response.magic_cookie,
            &response.transaction_id,
        )
        .unwrap();
        assert_eq!(mapped.address, client_addr.ip());
        assert_eq!(mapped.port, client_addr.port());
    }

    #[test]
    fn test_build_response_ipv6() {
        let transaction_id = [0xAA; 12];
        let request = StunMessage::new(StunMessageType::Request, transaction_id);
        let request_bytes = request.encode();

        let client_addr: SocketAddr = "[::1]:9999".parse().unwrap();
        let response_bytes = build_response(&request_bytes, client_addr).unwrap();

        let response = StunMessage::decode(&response_bytes).unwrap();
        assert_eq!(response.message_type, StunMessageType::Response);

        let mapped = MappedAddress::decode_xor(
            &response.attributes[0].attribute_value,
            response.magic_cookie,
            &response.transaction_id,
        )
        .unwrap();
        assert_eq!(mapped.address, client_addr.ip());
        assert_eq!(mapped.port, client_addr.port());
    }

    #[test]
    fn test_build_response_rejects_non_request() {
        // A response message should not get a response.
        let transaction_id = [1; 12];
        let msg = StunMessage::new(StunMessageType::Response, transaction_id);
        let bytes = msg.encode();

        let client_addr: SocketAddr = "10.0.0.1:1234".parse().unwrap();
        assert!(build_response(&bytes, client_addr).is_none());
    }

    #[test]
    fn test_build_response_rejects_invalid_data() {
        let client_addr: SocketAddr = "10.0.0.1:1234".parse().unwrap();
        assert!(build_response(&[0, 1, 2], client_addr).is_none());
    }

    #[test]
    fn test_build_response_rejects_empty() {
        let client_addr: SocketAddr = "10.0.0.1:1234".parse().unwrap();
        assert!(build_response(&[], client_addr).is_none());
    }

    #[test]
    fn test_server_state_default() {
        let state = ServerState::default();
        assert_eq!(state.request_count.load(Ordering::Relaxed), 0);
        assert_eq!(state.error_count.load(Ordering::Relaxed), 0);
        assert!(!state.timezone.is_empty());
    }

    #[test]
    fn test_build_response_with_multiple_attributes() {
        let transaction_id = [5; 12];
        let mut request = StunMessage::new(StunMessageType::Request, transaction_id);
        let attr = StunAttribute::new(StunAttributeType::Software, b"test-client".to_vec());
        request.add_attribute(attr);
        let request_bytes = request.encode();

        let client_addr: SocketAddr = "172.16.0.1:8080".parse().unwrap();
        let response_bytes = build_response(&request_bytes, client_addr).unwrap();

        let response = StunMessage::decode(&response_bytes).unwrap();
        assert_eq!(response.message_type, StunMessageType::Response);
        assert_eq!(response.transaction_id, transaction_id);
    }

    #[test]
    fn test_build_response_with_port_zero() {
        let transaction_id = [7; 12];
        let request = StunMessage::new(StunMessageType::Request, transaction_id);
        let request_bytes = request.encode();

        let client_addr: SocketAddr = "192.168.1.1:0".parse().unwrap();
        let response_bytes = build_response(&request_bytes, client_addr).unwrap();

        let response = StunMessage::decode(&response_bytes).unwrap();
        let mapped = MappedAddress::decode_xor(
            &response.attributes[0].attribute_value,
            response.magic_cookie,
            &response.transaction_id,
        )
        .unwrap();
        assert_eq!(mapped.port, 0);
    }

    #[test]
    fn test_server_state_concurrent_counters() {
        use std::thread;

        let state = Arc::new(ServerState::new());

        let mut handles = vec![];
        for _ in 0..10 {
            let state_clone = state.clone();
            handles.push(thread::spawn(move || {
                for _ in 0..100 {
                    state_clone.inc_requests();
                    state_clone.inc_errors();
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(state.request_count.load(Ordering::Relaxed), 1000);
        assert_eq!(state.error_count.load(Ordering::Relaxed), 1000);
    }
}
