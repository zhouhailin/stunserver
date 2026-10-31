//! STUN Server implementation.
//!
//! High-performance async UDP server using Tokio. Handles STUN Binding
//! requests by responding with XOR-MAPPED-ADDRESS containing the client's
//! reflexive address.

use crate::auth::{AuthOutcome, AuthService};
use crate::config::ServerConfig;
use chrono::Utc;
use log::{debug, error, info, trace, warn};
use std::fmt;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use stun_protocol::{
    describe_message, is_comprehension_required, ErrorCode, MappedAddress, StunAttribute,
    StunAttributeType, StunMessage, StunMessageType, UnknownAttributes,
};
use tokio::net::UdpSocket;
use tokio::signal;
use tokio::sync::Notify;

/// Size of the pre-allocated receive buffer (64 KB, well above STUN max).
const RECV_BUF_SIZE: usize = 65536;

/// SOFTWARE attribute value, identical across every response.
pub const SOFTWARE: &str = concat!("stunserver ", env!("CARGO_PKG_VERSION"));

/// Reason a datagram is silently discarded before authentication.
///
/// The variants carry lengths and type codes only; raw payload bytes and
/// secrets are never included (FR-013, FR-033).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntakeRejection {
    /// The datagram could not be decoded; the message names lengths or header
    /// fields only.
    Decode(String),
    /// The message is not a Binding Request.
    NotBindingRequest,
    /// FINGERPRINT is duplicated, non-final, or has a bad CRC.
    Fingerprint {
        attribute_type: u16,
        value_len: usize,
        reason: &'static str,
    },
    /// MESSAGE-INTEGRITY is duplicated or has the wrong length.
    MessageIntegrity {
        attribute_type: u16,
        value_len: usize,
        reason: &'static str,
    },
}

impl fmt::Display for IntakeRejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Decode(reason) => write!(f, "malformed message: {reason}"),
            Self::NotBindingRequest => write!(f, "not a Binding Request"),
            Self::Fingerprint {
                attribute_type,
                value_len,
                reason,
            } => write!(
                f,
                "FINGERPRINT(0x{attribute_type:04X}, len={value_len}): {reason}"
            ),
            Self::MessageIntegrity {
                attribute_type,
                value_len,
                reason,
            } => write!(
                f,
                "MESSAGE-INTEGRITY(0x{attribute_type:04X}, len={value_len}): {reason}"
            ),
        }
    }
}

/// Validate a datagram up to, but not including, the authentication stage.
///
/// Pipeline order (FR-008-FR-013): header decode and Binding Request check,
/// then FINGERPRINT position/length/CRC, then MESSAGE-INTEGRITY length and
/// uniqueness. Returns the decoded request when it may be processed further.
pub(crate) fn intake(encoded: &[u8]) -> Result<StunMessage, IntakeRejection> {
    let message =
        StunMessage::decode(encoded).map_err(|error| IntakeRejection::Decode(error.to_string()))?;

    if message.message_type != StunMessageType::Request {
        return Err(IntakeRejection::NotBindingRequest);
    }

    let fingerprint_type = StunAttributeType::Fingerprint as u16;
    let fingerprint_count = message.attribute_count(fingerprint_type);
    if fingerprint_count > 0 {
        let value_len = message
            .attributes
            .iter()
            .find(|attr| attr.raw_type == fingerprint_type)
            .map(|attr| attr.attribute_value.len())
            .unwrap_or(0);
        if fingerprint_count > 1 {
            return Err(IntakeRejection::Fingerprint {
                attribute_type: fingerprint_type,
                value_len,
                reason: "duplicate attribute",
            });
        }
        if message.attributes.last().map(|attr| attr.raw_type) != Some(fingerprint_type) {
            return Err(IntakeRejection::Fingerprint {
                attribute_type: fingerprint_type,
                value_len,
                reason: "not the final attribute",
            });
        }
        if !StunMessage::verify_message_fingerprint(encoded) {
            return Err(IntakeRejection::Fingerprint {
                attribute_type: fingerprint_type,
                value_len,
                reason: "CRC mismatch",
            });
        }
    }

    let integrity_type = StunAttributeType::MessageIntegrity as u16;
    let integrity_count = message.attribute_count(integrity_type);
    if integrity_count > 0 {
        let value_len = message
            .attributes
            .iter()
            .find(|attr| attr.raw_type == integrity_type)
            .map(|attr| attr.attribute_value.len())
            .unwrap_or(0);
        if integrity_count > 1 {
            return Err(IntakeRejection::MessageIntegrity {
                attribute_type: integrity_type,
                value_len,
                reason: "duplicate attribute",
            });
        }
        if value_len != stun_protocol::integrity::MESSAGE_INTEGRITY_LEN {
            return Err(IntakeRejection::MessageIntegrity {
                attribute_type: integrity_type,
                value_len,
                reason: "value must be 20 bytes",
            });
        }
    }

    Ok(message)
}

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
    /// Authentication stage configuration and precomputed keys.
    pub auth: Arc<AuthService>,
}

impl ServerState {
    /// Create a new server state with authentication disabled.
    pub fn new() -> Self {
        Self::with_auth(AuthService::disabled())
    }

    /// Create a new server state with an explicit authentication service.
    pub fn with_auth(auth: AuthService) -> Self {
        let timezone = chrono::Local::now().offset().to_string();
        Self {
            start_time: Utc::now(),
            timezone,
            request_count: AtomicU64::new(0),
            error_count: AtomicU64::new(0),
            auth: Arc::new(auth),
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

/// Process a single STUN request and produce a response buffer with
/// authentication disabled.
///
/// Parses the incoming UDP payload as a STUN Binding Request, validates it,
/// and constructs a Success Response with XOR-MAPPED-ADDRESS reflecting
/// the client's source address.
///
/// Returns `None` if the request is invalid (not a STUN message, wrong type, etc.).
pub fn build_response(buf: &[u8], client_addr: SocketAddr) -> Option<Vec<u8>> {
    build_response_with_auth(buf, client_addr, &AuthService::disabled())
}

/// Process a single STUN request under the configured authentication stage.
pub fn build_response_with_auth(
    buf: &[u8],
    client_addr: SocketAddr,
    auth: &AuthService,
) -> Option<Vec<u8>> {
    let request = intake(buf).ok()?;
    Some(build_response_bytes(buf, &request, client_addr, auth).1)
}

/// Build the response message and its wire bytes for a decoded Binding Request.
fn build_response_bytes(
    encoded: &[u8],
    request: &StunMessage,
    client_addr: SocketAddr,
    auth: &AuthService,
) -> (StunMessage, Vec<u8>) {
    match auth.authenticate(encoded, request) {
        AuthOutcome::Rejected {
            status,
            challenge,
            reason,
        } => {
            debug!("Authentication rejected for {}: {}", client_addr, reason);
            let response = build_auth_error_response(request, status, challenge, auth);
            let bytes = response.encode();
            (response, bytes)
        }
        AuthOutcome::Authenticated(key) => {
            let response = response_for_request(request, client_addr);
            sign_response(response, &key)
        }
        AuthOutcome::Disabled => {
            let response = response_for_request(request, client_addr);
            let bytes = response.encode();
            (response, bytes)
        }
    }
}

/// Build the response for a decoded Binding Request.
///
/// Returns a Binding Success Response when every comprehension-required
/// attribute is supported, or a 420 Binding Error Response listing the
/// unsupported attribute types otherwise (RFC 5389 section 7.3.1).
pub(crate) fn response_for_request(request: &StunMessage, client_addr: SocketAddr) -> StunMessage {
    let unknown = unsupported_required_attributes(request);

    if unknown.is_empty() {
        build_success_response(request, client_addr)
    } else {
        build_unknown_attribute_response(request, unknown)
    }
}

/// Append SOFTWARE to a response.
fn add_software(response: &mut StunMessage) {
    response.add_attribute(StunAttribute::new(
        StunAttributeType::Software,
        SOFTWARE.as_bytes().to_vec(),
    ));
}

/// Append a terminal MESSAGE-INTEGRITY attribute and compute its digest.
fn sign_response(mut response: StunMessage, key: &[u8]) -> (StunMessage, Vec<u8>) {
    response.add_attribute(StunAttribute::new(
        StunAttributeType::MessageIntegrity,
        vec![0u8; stun_protocol::integrity::MESSAGE_INTEGRITY_LEN],
    ));
    let mi_offset = *response
        .attribute_offsets()
        .last()
        .expect("MESSAGE-INTEGRITY attribute is present");
    let mut bytes = response.encode();
    let digest = stun_protocol::integrity::hmac_sha1(
        key,
        &stun_protocol::integrity::hmac_input(&bytes, mi_offset),
    );
    bytes[mi_offset + 4..mi_offset + 4 + stun_protocol::integrity::MESSAGE_INTEGRITY_LEN]
        .copy_from_slice(&digest);
    (response, bytes)
}

/// Build a 400/401/438 error response. When `challenge` is set the response
/// carries REALM and NONCE; no failure response is ever signed.
fn build_auth_error_response(
    request: &StunMessage,
    status: u16,
    challenge: bool,
    auth: &AuthService,
) -> StunMessage {
    let mut response = StunMessage::new(StunMessageType::ErrorIndication, request.transaction_id);

    let reason = match status {
        400 => "Bad Request",
        401 => "Unauthorized",
        438 => "Stale Nonce",
        _ => "Error",
    };
    let code = ErrorCode::new(4, (status % 100) as u8, reason.to_string());
    response.add_attribute(StunAttribute::new(
        StunAttributeType::ErrorCode,
        code.encode_value(),
    ));

    if challenge {
        response.add_attribute(StunAttribute::new(
            StunAttributeType::Realm,
            auth.realm().as_bytes().to_vec(),
        ));
        response.add_attribute(StunAttribute::new(
            StunAttributeType::Nonce,
            auth.issue_nonce().into_bytes(),
        ));
    }

    add_software(&mut response);
    response
}

/// RFC 5780 behavior-discovery attributes this single-address server rejects.
const REJECTED_REQUIRED: &[u16] = &[
    StunAttributeType::ChangeRequest as u16,
    StunAttributeType::Padding as u16,
    StunAttributeType::ResponsePort as u16,
];

/// Collect comprehension-required attribute types this server cannot honor,
/// preserving first-seen order and de-duplicating.
///
/// Known attributes (including authentication attributes and the
/// not-acted-upon MAPPED-ADDRESS, ERROR-CODE, UNKNOWN-ATTRIBUTES, and
/// ALTERNATE-SERVER) are ignored. Attributes after MESSAGE-INTEGRITY are
/// excluded from classification (RFC 5389 §15.4).
fn unsupported_required_attributes(request: &StunMessage) -> Vec<u16> {
    let mut types: Vec<u16> = Vec::new();

    for attr in &request.attributes {
        if attr.raw_type == StunAttributeType::MessageIntegrity as u16 {
            break;
        }
        if !is_comprehension_required(attr.raw_type) {
            continue;
        }
        let recognized = StunAttributeType::from_raw(attr.raw_type) != StunAttributeType::Unknown;
        let rejected = REJECTED_REQUIRED.contains(&attr.raw_type) || !recognized;
        if rejected && !types.contains(&attr.raw_type) {
            types.push(attr.raw_type);
        }
    }

    types
}

/// Build a 420 Unknown Attribute error response.
fn build_unknown_attribute_response(request: &StunMessage, unknown: Vec<u16>) -> StunMessage {
    let mut response = StunMessage::new(StunMessageType::ErrorIndication, request.transaction_id);

    let code = ErrorCode::new(4, 20, "Unknown Attribute".to_string());
    response.add_attribute(StunAttribute::new(
        StunAttributeType::ErrorCode,
        code.encode_value(),
    ));

    let list = UnknownAttributes::new(unknown);
    response.add_attribute(StunAttribute::new(
        StunAttributeType::UnknownAttributes,
        list.encode_value(),
    ));

    add_software(&mut response);
    response
}

/// Build the Binding Success Response for a decoded request.
fn build_success_response(request: &StunMessage, client_addr: SocketAddr) -> StunMessage {
    let mut response = StunMessage::new(StunMessageType::Response, request.transaction_id);

    // Create XOR-MAPPED-ADDRESS attribute with client's reflexive address.
    let mapped = MappedAddress::new(client_addr.ip(), client_addr.port());
    let xor_value = mapped.encode_xor(request.magic_cookie, &request.transaction_id);
    let attr = StunAttribute::new(StunAttributeType::XorMappedAddress, xor_value);
    response.add_attribute(attr);
    add_software(&mut response);

    response
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
    let listen_addr = config.socket_addr()?;

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

                            match intake(&data) {
                                Ok(request) => {
                                    trace!("Received: {:02x?}", data);
                                    debug!("Received: {}", describe_message(&request));

                                    let (response_msg, response) = build_response_bytes(
                                        &data,
                                        &request,
                                        client_addr,
                                        &state.auth,
                                    );
                                    debug!("Response: {}", describe_message(&response_msg));

                                    trace!("Response: {:02x?}", response);
                                    if let Err(e) = sock.send_to(&response, client_addr).await {
                                        state.inc_errors();
                                        warn!("Failed to send response to {}: {}", client_addr, e);
                                    }
                                }
                                Err(reason) => {
                                    state.inc_errors();
                                    debug!(
                                        "Discarded {} byte message from {}: {}",
                                        len, client_addr, reason
                                    );
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
    use stun_protocol::message::MAGIC_COOKIE;
    use stun_protocol::{describe_attribute, StunMessage, StunMessageType};

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
        assert_eq!(response.attributes.len(), 2);
        assert_eq!(
            response.attributes[0].attribute_type,
            StunAttributeType::XorMappedAddress
        );
        assert_eq!(
            response.attributes[1].attribute_type,
            StunAttributeType::Software
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

    fn txn_hex(transaction_id: &[u8; 12]) -> String {
        transaction_id
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect()
    }

    #[test]
    fn test_describe_message_empty_attributes() {
        let transaction_id = [0u8; 12];
        let msg = StunMessage::new(StunMessageType::Request, transaction_id);
        assert_eq!(
            describe_message(&msg),
            format!("Binding Request txn={} attrs=[]", txn_hex(&transaction_id))
        );
    }

    #[test]
    fn test_describe_message_with_software() {
        let transaction_id = [
            0x09, 0xe9, 0x6d, 0x1c, 0x0e, 0x8a, 0xe3, 0x18, 0x00, 0x00, 0x00, 0x00,
        ];
        let mut msg = StunMessage::new(StunMessageType::Request, transaction_id);
        msg.add_attribute(StunAttribute::new(
            StunAttributeType::Software,
            b"test-client".to_vec(),
        ));

        assert_eq!(
            describe_message(&msg),
            format!(
                "Binding Request txn={} attrs=[SOFTWARE=\"test-client\"]",
                txn_hex(&transaction_id)
            )
        );
    }

    #[test]
    fn test_describe_message_xor_mapped_ipv4() {
        let transaction_id = [0x11; 12];
        let client_addr: SocketAddr = "192.0.2.1:5000".parse().unwrap();
        let mapped = MappedAddress::new(client_addr.ip(), client_addr.port());
        let xor_value = mapped.encode_xor(MAGIC_COOKIE, &transaction_id);

        let mut msg = StunMessage::new(StunMessageType::Response, transaction_id);
        msg.add_attribute(StunAttribute::new(
            StunAttributeType::XorMappedAddress,
            xor_value,
        ));

        assert_eq!(
            describe_message(&msg),
            format!(
                "Binding Success Response txn={} attrs=[XOR-MAPPED-ADDRESS=192.0.2.1:5000]",
                txn_hex(&transaction_id)
            )
        );
    }

    #[test]
    fn test_describe_message_xor_mapped_ipv6() {
        let transaction_id = [0x22; 12];
        let client_addr: SocketAddr = "[2001:db8::1]:9999".parse().unwrap();
        let mapped = MappedAddress::new(client_addr.ip(), client_addr.port());
        let xor_value = mapped.encode_xor(MAGIC_COOKIE, &transaction_id);

        let mut msg = StunMessage::new(StunMessageType::Response, transaction_id);
        msg.add_attribute(StunAttribute::new(
            StunAttributeType::XorMappedAddress,
            xor_value,
        ));

        assert_eq!(
            describe_message(&msg),
            format!(
                "Binding Success Response txn={} attrs=[XOR-MAPPED-ADDRESS=[2001:db8::1]:9999]",
                txn_hex(&transaction_id)
            )
        );
    }

    #[test]
    fn test_describe_attribute_unknown_type_uses_raw_code() {
        let raw = [0xAB, 0xCD, 0x00, 0x04, 0xDE, 0xAD, 0xBE, 0xEF];
        let attr = StunAttribute::decode(&raw).unwrap();
        let transaction_id = [0u8; 12];

        assert_eq!(
            describe_attribute(&attr, MAGIC_COOKIE, &transaction_id),
            "0xABCD(len=4)"
        );
    }

    #[test]
    fn test_describe_attribute_address_decode_failure() {
        // Address family 0x03 is invalid, so decoding must fail.
        let attr = StunAttribute::new(
            StunAttributeType::XorMappedAddress,
            vec![0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        );
        let transaction_id = [0u8; 12];

        assert_eq!(
            describe_attribute(&attr, MAGIC_COOKIE, &transaction_id),
            "XOR-MAPPED-ADDRESS(len=8)"
        );
    }

    #[test]
    fn test_describe_attribute_known_non_address() {
        let attr = StunAttribute::new(StunAttributeType::ErrorCode, vec![0x00, 0x00, 0x04, 0x00]);
        let transaction_id = [0u8; 12];

        assert_eq!(
            describe_attribute(&attr, MAGIC_COOKIE, &transaction_id),
            "ERROR-CODE=400 \"\""
        );
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

    fn raw_attribute(raw_type: u16, value: &[u8]) -> StunAttribute {
        let mut bytes = Vec::with_capacity(4 + value.len());
        bytes.extend_from_slice(&raw_type.to_be_bytes());
        bytes.extend_from_slice(&(value.len() as u16).to_be_bytes());
        bytes.extend_from_slice(value);
        StunAttribute::decode(&bytes).unwrap()
    }

    #[test]
    fn test_build_response_unknown_required_attribute() {
        use stun_protocol::{ErrorCode, StunAttributeType, UnknownAttributes};

        let transaction_id = [0x31; 12];
        let mut request = StunMessage::new(StunMessageType::Request, transaction_id);
        request.add_attribute(raw_attribute(0x0024, &[0x6e, 0x00, 0x01, 0xff]));
        let request_bytes = request.encode();

        let client_addr: SocketAddr = "192.0.2.10:4000".parse().unwrap();
        let response_bytes = build_response(&request_bytes, client_addr).unwrap();
        let response = StunMessage::decode(&response_bytes).unwrap();

        assert_eq!(response.message_type, StunMessageType::ErrorIndication);
        assert_eq!(response.transaction_id, transaction_id);
        assert_eq!(response.magic_cookie, request.magic_cookie);
        assert_eq!(response.attributes.len(), 3);
        assert_eq!(
            response.attributes[0].attribute_type,
            StunAttributeType::ErrorCode
        );
        assert_eq!(
            response.attributes[1].attribute_type,
            StunAttributeType::UnknownAttributes
        );
        assert_eq!(response.attributes[1].raw_type, 0x000A);
        assert_eq!(
            response.attributes[2].attribute_type,
            StunAttributeType::Software
        );
        assert!(response
            .attributes
            .iter()
            .all(|attr| attr.attribute_type != StunAttributeType::XorMappedAddress));

        let code = ErrorCode::decode_value(&response.attributes[0].attribute_value).unwrap();
        assert_eq!(code.class, 4);
        assert_eq!(code.number, 20);
        assert_eq!(code.reason, "Unknown Attribute");

        let unknown =
            UnknownAttributes::decode_value(&response.attributes[1].attribute_value).unwrap();
        assert_eq!(unknown.types, vec![0x0024]);
    }

    #[test]
    fn test_build_response_unknown_required_deduplicates() {
        use stun_protocol::UnknownAttributes;

        let transaction_id = [0x32; 12];
        let mut request = StunMessage::new(StunMessageType::Request, transaction_id);
        request.add_attribute(raw_attribute(0x0024, &[0, 0, 0, 0]));
        request.add_attribute(raw_attribute(0x0025, &[0, 0, 0, 0]));
        request.add_attribute(raw_attribute(0x0024, &[0, 0, 0, 0]));
        let request_bytes = request.encode();

        let client_addr: SocketAddr = "192.0.2.11:4000".parse().unwrap();
        let response_bytes = build_response(&request_bytes, client_addr).unwrap();
        let response = StunMessage::decode(&response_bytes).unwrap();

        assert_eq!(response.message_type, StunMessageType::ErrorIndication);
        let unknown =
            UnknownAttributes::decode_value(&response.attributes[1].attribute_value).unwrap();
        assert_eq!(unknown.types, vec![0x0024, 0x0025]);
    }

    #[test]
    fn test_build_response_ignores_unknown_optional_attribute() {
        let transaction_id = [0x33; 12];
        let mut request = StunMessage::new(StunMessageType::Request, transaction_id);
        request.add_attribute(raw_attribute(0x80FF, &[0xde, 0xad, 0xbe, 0xef]));
        let request_bytes = request.encode();

        let client_addr: SocketAddr = "192.0.2.12:4000".parse().unwrap();
        let response_bytes = build_response(&request_bytes, client_addr).unwrap();
        let response = StunMessage::decode(&response_bytes).unwrap();

        assert_eq!(response.message_type, StunMessageType::Response);
        assert_eq!(response.attributes.len(), 2);
        assert_eq!(
            response.attributes[0].attribute_type,
            StunAttributeType::XorMappedAddress
        );
        assert_eq!(
            response.attributes[1].attribute_type,
            StunAttributeType::Software
        );
    }

    #[test]
    fn test_build_response_lists_only_required_attributes() {
        use stun_protocol::UnknownAttributes;

        let transaction_id = [0x34; 12];
        let mut request = StunMessage::new(StunMessageType::Request, transaction_id);
        request.add_attribute(raw_attribute(0x80FF, &[0x00, 0x00, 0x00, 0x00]));
        request.add_attribute(raw_attribute(0x0024, &[0x00, 0x00, 0x00, 0x00]));
        let request_bytes = request.encode();

        let client_addr: SocketAddr = "192.0.2.13:4000".parse().unwrap();
        let response_bytes = build_response(&request_bytes, client_addr).unwrap();
        let response = StunMessage::decode(&response_bytes).unwrap();

        assert_eq!(response.message_type, StunMessageType::ErrorIndication);
        let unknown =
            UnknownAttributes::decode_value(&response.attributes[1].attribute_value).unwrap();
        assert_eq!(unknown.types, vec![0x0024]);
    }
}
