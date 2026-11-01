//! Integration tests for STUN Client.
//!
//! Tests cover request building, response parsing, timeout handling,
//! authentication flows, and error cases by running a mock STUN server
//! alongside the client.

use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, Once};
use std::time::Duration;
use stun_protocol::integrity::{fingerprint, long_term_key, short_term_key};
use stun_protocol::{
    MappedAddress, StunAttribute, StunAttributeType, StunMessage, StunMessageType,
};
use stunclient::client;
use tokio::net::UdpSocket;

// ---------------------------------------------------------------------------
// Log capture (SC-010 redaction assertions)
// ---------------------------------------------------------------------------

static CAPTURED: Mutex<Vec<String>> = Mutex::new(Vec::new());
static LOGGER_INIT: Once = Once::new();

struct CaptureLogger;

impl log::Log for CaptureLogger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::Level::Trace
    }

    fn log(&self, record: &log::Record<'_>) {
        CAPTURED.lock().unwrap().push(record.args().to_string());
    }

    fn flush(&self) {}
}

fn install_capture_logger() {
    LOGGER_INIT.call_once(|| {
        let _ = log::set_logger(&CaptureLogger);
        log::set_max_level(log::LevelFilter::Trace);
    });
}

// ---------------------------------------------------------------------------
// Mock server helpers
// ---------------------------------------------------------------------------

/// Spawn a mock STUN server that responds to binding requests.
/// Returns the server's listen address.
async fn spawn_mock_server(respond_with: MockResponse) -> SocketAddr {
    let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let addr = socket.local_addr().unwrap();
    let socket = Arc::new(socket);

    tokio::spawn(async move {
        let alt_socket = if matches!(respond_with, MockResponse::WrongSource) {
            Some(UdpSocket::bind("127.0.0.1:0").await.unwrap())
        } else {
            None
        };

        let mut buf = vec![0u8; 65536];
        // Handle up to 10 requests then exit.
        for _ in 0..10 {
            let (len, from) = match socket.recv_from(&mut buf).await {
                Ok(r) => r,
                Err(_) => break,
            };

            let response_bytes = match &respond_with {
                MockResponse::Valid => {
                    let request = StunMessage::decode(&buf[..len]).unwrap();
                    success_response(&request, from, false)
                }
                MockResponse::ValidWithFingerprint => {
                    let request = StunMessage::decode(&buf[..len]).unwrap();
                    success_response(&request, from, true)
                }
                MockResponse::BadFingerprintCrc => {
                    let request = StunMessage::decode(&buf[..len]).unwrap();
                    let mut bytes = success_response(&request, from, true);
                    let message = StunMessage::decode(&bytes).unwrap();
                    let offset = message
                        .attribute_offset(StunAttributeType::Fingerprint as u16)
                        .unwrap();
                    bytes[offset + 7] ^= 0xFF;
                    bytes
                }
                MockResponse::DuplicateFingerprint => {
                    let request = StunMessage::decode(&buf[..len]).unwrap();
                    let mut bytes = success_response(&request, from, true);
                    let new_length = u16::from_be_bytes([bytes[2], bytes[3]]).saturating_add(8);
                    bytes[2..4].copy_from_slice(&new_length.to_be_bytes());
                    let crc = fingerprint(&bytes);
                    bytes.extend_from_slice(&[0x80, 0x28, 0x00, 0x04]);
                    bytes.extend_from_slice(&crc.to_be_bytes());
                    bytes
                }
                MockResponse::NonTerminalFingerprint => {
                    let request = StunMessage::decode(&buf[..len]).unwrap();
                    let mut bytes = success_response(&request, from, true);
                    let new_length = u16::from_be_bytes([bytes[2], bytes[3]]).saturating_add(8);
                    bytes[2..4].copy_from_slice(&new_length.to_be_bytes());
                    bytes.extend_from_slice(&[0x80, 0x22, 0x00, 0x01, b'x', 0, 0, 0]);
                    let message = StunMessage::decode(&bytes).unwrap();
                    let offset = message
                        .attribute_offset(StunAttributeType::Fingerprint as u16)
                        .unwrap();
                    let crc = fingerprint(&bytes[..offset]);
                    bytes[offset + 4..offset + 8].copy_from_slice(&crc.to_be_bytes());
                    bytes
                }
                MockResponse::TruncatedFingerprint => {
                    let request = StunMessage::decode(&buf[..len]).unwrap();
                    let mut response =
                        StunMessage::new(StunMessageType::Response, request.transaction_id);
                    add_mapped_address(&mut response, from);
                    // Two-byte FINGERPRINT value; the datagram is otherwise valid.
                    response.add_attribute(StunAttribute::new(
                        StunAttributeType::Fingerprint,
                        vec![0x00, 0x00],
                    ));
                    response.encode()
                }
                MockResponse::CutOffFingerprint => {
                    let request = StunMessage::decode(&buf[..len]).unwrap();
                    let mut bytes = success_response(&request, from, true);
                    // Drop the final two bytes of the FINGERPRINT value.
                    bytes.truncate(bytes.len() - 2);
                    bytes
                }
                MockResponse::NoAttribute => {
                    let request = StunMessage::decode(&buf[..len]).unwrap();
                    StunMessage::new(StunMessageType::Response, request.transaction_id).encode()
                }
                MockResponse::Silent => {
                    // Don't respond at all.
                    continue;
                }
                MockResponse::ServerError {
                    class,
                    number,
                    types,
                    include_list,
                } => {
                    let request = StunMessage::decode(&buf[..len]).unwrap();
                    let mut response =
                        StunMessage::new(StunMessageType::ErrorResponse, request.transaction_id);
                    // Build the value directly so invalid codes can be tested.
                    let mut value = vec![0x00, 0x00, *class, *number];
                    value.extend_from_slice(b"unknown attribute");
                    response.add_attribute(StunAttribute::new(StunAttributeType::ErrorCode, value));
                    if *include_list {
                        let list = stun_protocol::UnknownAttributes::new(types.clone());
                        response.add_attribute(StunAttribute::new(
                            StunAttributeType::UnknownAttributes,
                            list.encode_value(),
                        ));
                    }
                    response.encode()
                }
                MockResponse::SuccessWithRawAttribute { raw_type, value } => {
                    let request = StunMessage::decode(&buf[..len]).unwrap();
                    let mut response =
                        StunMessage::new(StunMessageType::Response, request.transaction_id);
                    add_mapped_address(&mut response, from);
                    response.add_attribute(raw_attribute(*raw_type, value));
                    response.encode()
                }
                MockResponse::WrongTransactionId => {
                    let wrong_id = [0xAA; 12];
                    let mut response = StunMessage::new(StunMessageType::Response, wrong_id);
                    add_mapped_address(&mut response, from);
                    response.encode()
                }
                MockResponse::Malformed => b"not a stun message".to_vec(),
                MockResponse::BadXorLength => {
                    let request = StunMessage::decode(&buf[..len]).unwrap();
                    let mut response =
                        StunMessage::new(StunMessageType::Response, request.transaction_id);
                    // IPv4 family advertised with a 20-byte value.
                    let mut value = vec![0x00, 0x01, 0x00, 0x00];
                    value.extend_from_slice(&[0u8; 16]);
                    response.add_attribute(StunAttribute::new(
                        StunAttributeType::XorMappedAddress,
                        value,
                    ));
                    response.encode()
                }
                MockResponse::TruncatedXor => {
                    let request = StunMessage::decode(&buf[..len]).unwrap();
                    let mut response =
                        StunMessage::new(StunMessageType::Response, request.transaction_id);
                    // IPv4 family advertised with only the 4-byte prefix.
                    response.add_attribute(StunAttribute::new(
                        StunAttributeType::XorMappedAddress,
                        vec![0x00, 0x01, 0x00, 0x00],
                    ));
                    response.encode()
                }
                MockResponse::WrongSource => {
                    let request = StunMessage::decode(&buf[..len]).unwrap();
                    success_response(&request, from, false)
                }
            };

            let sender: &UdpSocket = alt_socket.as_ref().unwrap_or(&socket);
            let _ = sender.send_to(&response_bytes, from).await;
        }
    });

    addr
}

enum MockResponse {
    Valid,
    ValidWithFingerprint,
    BadFingerprintCrc,
    DuplicateFingerprint,
    NonTerminalFingerprint,
    TruncatedFingerprint,
    CutOffFingerprint,
    NoAttribute,
    Silent,
    ServerError {
        class: u8,
        number: u8,
        types: Vec<u16>,
        include_list: bool,
    },
    SuccessWithRawAttribute {
        raw_type: u16,
        value: Vec<u8>,
    },
    WrongTransactionId,
    WrongSource,
    Malformed,
    BadXorLength,
    TruncatedXor,
}

fn add_mapped_address(response: &mut StunMessage, from: SocketAddr) {
    let mapped = MappedAddress::new(from.ip(), from.port());
    let xor_value = mapped.encode_xor(response.magic_cookie, &response.transaction_id);
    response.add_attribute(StunAttribute::new(
        StunAttributeType::XorMappedAddress,
        xor_value,
    ));
}

fn success_response(request: &StunMessage, from: SocketAddr, with_fingerprint: bool) -> Vec<u8> {
    let mut response = StunMessage::new(StunMessageType::Response, request.transaction_id);
    add_mapped_address(&mut response, from);
    response.add_attribute(StunAttribute::new(
        StunAttributeType::Software,
        b"mock-server".to_vec(),
    ));
    if with_fingerprint {
        response.encode_with_fingerprint()
    } else {
        response.encode()
    }
}

fn signed_success_response(request: &StunMessage, from: SocketAddr, key: &[u8]) -> Vec<u8> {
    let mut response = StunMessage::new(StunMessageType::Response, request.transaction_id);
    add_mapped_address(&mut response, from);
    response.add_attribute(StunAttribute::new(
        StunAttributeType::Software,
        b"mock-server".to_vec(),
    ));
    response.encode_with_integrity(key)
}

fn challenge_response(
    transaction_id: [u8; 12],
    class: u8,
    number: u8,
    reason: &str,
    realm_and_nonce: Option<(&str, &str)>,
) -> Vec<u8> {
    let mut response = StunMessage::new(StunMessageType::ErrorResponse, transaction_id);
    let code = stun_protocol::ErrorCode::new(class, number, reason.to_string())
        .expect("test challenge code is valid");
    response.add_attribute(StunAttribute::new(
        StunAttributeType::ErrorCode,
        code.encode_value(),
    ));
    if let Some((realm, nonce)) = realm_and_nonce {
        response.add_attribute(StunAttribute::new(
            StunAttributeType::Realm,
            realm.as_bytes().to_vec(),
        ));
        response.add_attribute(StunAttribute::new(
            StunAttributeType::Nonce,
            nonce.as_bytes().to_vec(),
        ));
    }
    response.add_attribute(StunAttribute::new(
        StunAttributeType::Software,
        b"mock-server".to_vec(),
    ));
    response.encode()
}

fn signed_challenge_response(
    transaction_id: [u8; 12],
    class: u8,
    number: u8,
    reason: &str,
    realm_and_nonce: Option<(&str, &str)>,
    key: &[u8],
) -> Vec<u8> {
    let mut response = StunMessage::new(StunMessageType::ErrorResponse, transaction_id);
    let code = stun_protocol::ErrorCode::new(class, number, reason.to_string())
        .expect("test error code is valid");
    response.add_attribute(StunAttribute::new(
        StunAttributeType::ErrorCode,
        code.encode_value(),
    ));
    if let Some((realm, nonce)) = realm_and_nonce {
        response.add_attribute(StunAttribute::new(
            StunAttributeType::Realm,
            realm.as_bytes().to_vec(),
        ));
        response.add_attribute(StunAttribute::new(
            StunAttributeType::Nonce,
            nonce.as_bytes().to_vec(),
        ));
    }
    response.add_attribute(StunAttribute::new(
        StunAttributeType::Software,
        b"mock-server".to_vec(),
    ));
    response.encode_with_integrity(key)
}

fn raw_attribute(raw_type: u16, value: &[u8]) -> StunAttribute {
    let mut bytes = Vec::with_capacity(4 + value.len());
    bytes.extend_from_slice(&raw_type.to_be_bytes());
    bytes.extend_from_slice(&(value.len() as u16).to_be_bytes());
    bytes.extend_from_slice(value);
    StunAttribute::decode(&bytes).unwrap()
}

/// Append an attribute after MESSAGE-INTEGRITY, patching the header length.
/// The HMAC stays valid because RFC 5389 section 15.4 covers only the bytes up
/// to MESSAGE-INTEGRITY.
fn append_after_integrity(bytes: &mut Vec<u8>, attr: StunAttribute) {
    let encoded = attr.encode();
    let length = u16::from_be_bytes([bytes[2], bytes[3]]) + encoded.len() as u16;
    bytes[2..4].copy_from_slice(&length.to_be_bytes());
    bytes.extend_from_slice(&encoded);
}

fn text_attribute(message: &StunMessage, attribute_type: StunAttributeType) -> Option<&str> {
    message
        .attributes
        .iter()
        .find(|attr| attr.attribute_type == attribute_type)
        .and_then(|attr| std::str::from_utf8(&attr.attribute_value).ok())
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{:02x}", byte)).collect()
}

async fn bind_mock() -> (Arc<UdpSocket>, SocketAddr) {
    let socket = Arc::new(UdpSocket::bind("127.0.0.1:0").await.unwrap());
    let addr = socket.local_addr().unwrap();
    (socket, addr)
}

// ---------------------------------------------------------------------------
// Transaction IDs (FR-034, SC-006)
// ---------------------------------------------------------------------------

#[test]
fn test_generated_transaction_ids_are_random() {
    let mut seen = HashSet::new();
    let mut per_position: [HashSet<u8>; 12] = std::array::from_fn(|_| HashSet::new());

    for _ in 0..10_000 {
        let id = client::generate_transaction_id();
        assert_eq!(id.len(), 12);
        for (index, byte) in id.iter().enumerate() {
            per_position[index].insert(*byte);
        }
        assert!(seen.insert(id), "transaction ID repeated");
    }

    for (index, values) in per_position.iter().enumerate() {
        assert!(
            values.len() > 1,
            "byte {index} never varies; IDs are not random"
        );
    }
}

// ---------------------------------------------------------------------------
// Request shape (FR-035)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_binding_request_contains_software() {
    let (socket, server_addr) = bind_mock().await;
    let handle = tokio::spawn(async move {
        let mut buf = vec![0u8; 65536];
        let (len, from) = socket.recv_from(&mut buf).await.unwrap();
        let request = StunMessage::decode(&buf[..len]).unwrap();
        assert_eq!(
            text_attribute(&request, StunAttributeType::Software),
            Some(client::SOFTWARE)
        );
        assert_eq!(
            request.attribute_count(StunAttributeType::MessageIntegrity as u16),
            0
        );
        let response = success_response(&request, from, false);
        socket.send_to(&response, from).await.unwrap();
    });

    client::send_binding_request(server_addr, Duration::from_secs(2))
        .await
        .unwrap();
    handle.await.unwrap();
}

// ---------------------------------------------------------------------------
// FINGERPRINT validation (FR-036)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_valid_terminal_fingerprint_accepted() {
    let server_addr = spawn_mock_server(MockResponse::ValidWithFingerprint).await;

    let result = client::send_binding_request(server_addr, Duration::from_secs(2))
        .await
        .unwrap();

    assert!(result.mapped_address.port() > 0);
}

#[tokio::test]
async fn test_invalid_fingerprint_crc_is_discarded() {
    let server_addr = spawn_mock_server(MockResponse::BadFingerprintCrc).await;

    let err = client::send_binding_request(server_addr, Duration::from_millis(150))
        .await
        .unwrap_err();

    assert!(matches!(err, client::ClientError::Timeout(_)));
}

#[tokio::test]
async fn test_duplicate_fingerprint_is_discarded() {
    let server_addr = spawn_mock_server(MockResponse::DuplicateFingerprint).await;

    let err = client::send_binding_request(server_addr, Duration::from_millis(150))
        .await
        .unwrap_err();

    assert!(matches!(err, client::ClientError::Timeout(_)));
}

#[tokio::test]
async fn test_non_terminal_fingerprint_is_discarded() {
    let server_addr = spawn_mock_server(MockResponse::NonTerminalFingerprint).await;

    let err = client::send_binding_request(server_addr, Duration::from_millis(150))
        .await
        .unwrap_err();

    assert!(matches!(err, client::ClientError::Timeout(_)));
}

#[tokio::test]
async fn test_truncated_fingerprint_is_discarded() {
    // RFC 5389 section 15.5: a FINGERPRINT value is always 4 bytes. A shorter
    // value makes the response invalid, so the client must discard it and
    // retransmit instead of accepting the mapped address.
    let server_addr = spawn_mock_server(MockResponse::TruncatedFingerprint).await;

    let err = client::send_binding_request(server_addr, Duration::from_millis(150))
        .await
        .unwrap_err();

    assert!(matches!(err, client::ClientError::Timeout(_)));
}

#[tokio::test]
async fn test_fingerprint_datagram_truncation_is_discarded() {
    // A datagram cut off inside the FINGERPRINT value cannot be validated and
    // must be ignored.
    let server_addr = spawn_mock_server(MockResponse::CutOffFingerprint).await;

    let err = client::send_binding_request(server_addr, Duration::from_millis(150))
        .await
        .unwrap_err();

    assert!(matches!(err, client::ClientError::Timeout(_)));
}

// ---------------------------------------------------------------------------
// ERROR-CODE validation (FR-037, FR-038)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_error_class_above_range_is_discarded() {
    let server_addr = spawn_mock_server(MockResponse::ServerError {
        class: 7,
        number: 0,
        types: vec![],
        include_list: false,
    })
    .await;

    let err = client::send_binding_request(server_addr, Duration::from_millis(150))
        .await
        .unwrap_err();

    assert!(matches!(err, client::ClientError::Timeout(_)));
}

#[tokio::test]
async fn test_error_class_below_range_is_discarded() {
    let server_addr = spawn_mock_server(MockResponse::ServerError {
        class: 2,
        number: 0,
        types: vec![],
        include_list: false,
    })
    .await;

    let err = client::send_binding_request(server_addr, Duration::from_millis(150))
        .await
        .unwrap_err();

    assert!(matches!(err, client::ClientError::Timeout(_)));
}

#[tokio::test]
async fn test_error_number_above_99_is_discarded() {
    let server_addr = spawn_mock_server(MockResponse::ServerError {
        class: 4,
        number: 100,
        types: vec![],
        include_list: false,
    })
    .await;

    let err = client::send_binding_request(server_addr, Duration::from_millis(150))
        .await
        .unwrap_err();

    assert!(matches!(err, client::ClientError::Timeout(_)));
}

// ---------------------------------------------------------------------------
// Source address validation (FR-037)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_response_from_unexpected_source_is_discarded() {
    let server_addr = spawn_mock_server(MockResponse::WrongSource).await;

    let err = client::send_binding_request(server_addr, Duration::from_millis(150))
        .await
        .unwrap_err();

    assert!(matches!(err, client::ClientError::Timeout(_)));
}

#[tokio::test]
async fn test_binding_request_retransmits_lost_datagram() {
    let (socket, server_addr) = bind_mock().await;
    let handle = tokio::spawn(async move {
        let mut buf = vec![0u8; 65536];
        // Drop the first request to force a retransmission.
        let _ = socket.recv_from(&mut buf).await.unwrap();
        let (len, from) = socket.recv_from(&mut buf).await.unwrap();
        let request = StunMessage::decode(&buf[..len]).unwrap();
        let response = success_response(&request, from, false);
        socket.send_to(&response, from).await.unwrap();
    });

    let result = client::send_binding_request(server_addr, Duration::from_secs(2))
        .await
        .unwrap();

    assert!(result.mapped_address.port() > 0);
    handle.await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn test_binding_request_sends_rc_requests_and_waits_rm_rto() {
    let socket = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let server_addr = socket.local_addr().unwrap();
    let start = tokio::time::Instant::now();

    let err = client::send_binding_request(server_addr, Duration::from_secs(60))
        .await
        .unwrap_err();

    assert!(matches!(err, client::ClientError::Timeout(_)));
    // RFC 5389 section 7.2.1: the transaction fails 39.5 s after the first
    // transmission with RTO = 500 ms and Rm = 16.
    assert_eq!(start.elapsed(), Duration::from_millis(39_500));

    let mut buf = [0u8; 2048];
    let mut received = 0;
    while socket.try_recv_from(&mut buf).is_ok() {
        received += 1;
    }
    assert_eq!(received, client::MAX_REQUESTS);
}

#[tokio::test]
async fn test_binding_request_discards_wrong_transaction_id_then_accepts() {
    let (socket, server_addr) = bind_mock().await;
    let handle = tokio::spawn(async move {
        let mut buf = vec![0u8; 65536];
        let (_, from) = socket.recv_from(&mut buf).await.unwrap();
        let mut wrong = StunMessage::new(StunMessageType::Response, [0xAA; 12]);
        add_mapped_address(&mut wrong, from);
        socket.send_to(&wrong.encode(), from).await.unwrap();

        let (len, from) = socket.recv_from(&mut buf).await.unwrap();
        let request = StunMessage::decode(&buf[..len]).unwrap();
        let response = success_response(&request, from, false);
        socket.send_to(&response, from).await.unwrap();
    });

    let result = client::send_binding_request(server_addr, Duration::from_secs(2))
        .await
        .unwrap();

    assert!(result.mapped_address.port() > 0);
    handle.await.unwrap();
}

// ---------------------------------------------------------------------------
// Existing behavior regressions
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_binding_request_success() {
    let server_addr = spawn_mock_server(MockResponse::Valid).await;

    let result = client::send_binding_request(server_addr, Duration::from_secs(5))
        .await
        .unwrap();

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
async fn test_binding_request_missing_attribute_is_discarded() {
    let server_addr = spawn_mock_server(MockResponse::NoAttribute).await;

    let err = client::send_binding_request(server_addr, Duration::from_millis(150))
        .await
        .unwrap_err();

    assert!(matches!(err, client::ClientError::Timeout(_)));
}

#[tokio::test]
async fn test_binding_request_unreachable_server() {
    let unreachable: SocketAddr = "192.0.2.1:3478".parse().unwrap();

    let result = client::send_binding_request(unreachable, Duration::from_millis(200)).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        client::ClientError::Timeout(_) | client::ClientError::Connection(_) => {}
        other => panic!("unexpected error: {:?}", other),
    }
}

#[tokio::test]
async fn test_binding_request_ipv6() {
    let socket = UdpSocket::bind("[::1]:0").await.unwrap();
    let server_addr = socket.local_addr().unwrap();
    let socket = Arc::new(socket);

    tokio::spawn(async move {
        let mut buf = vec![0u8; 65536];
        let (len, from) = socket.recv_from(&mut buf).await.unwrap();
        let request = StunMessage::decode(&buf[..len]).unwrap();
        let response = success_response(&request, from, false);
        let _ = socket.send_to(&response, from).await;
    });

    let result = client::send_binding_request(server_addr, Duration::from_secs(5))
        .await
        .unwrap();

    assert!(result.mapped_address.ip().is_loopback());
}

#[tokio::test]
async fn test_binding_request_420_with_unknown_attributes() {
    let server_addr = spawn_mock_server(MockResponse::ServerError {
        class: 4,
        number: 20,
        types: vec![0x0024],
        include_list: true,
    })
    .await;

    let err = client::send_binding_request(server_addr, Duration::from_secs(5))
        .await
        .unwrap_err();

    match err {
        client::ClientError::ServerError {
            class,
            number,
            reason,
            unknown_attributes,
        } => {
            assert_eq!(class, 4);
            assert_eq!(number, 20);
            assert_eq!(reason, "unknown attribute");
            assert_eq!(unknown_attributes, vec![0x0024]);
        }
        other => panic!("unexpected error: {:?}", other),
    }
}

#[tokio::test]
async fn test_binding_request_420_without_unknown_attributes() {
    let server_addr = spawn_mock_server(MockResponse::ServerError {
        class: 4,
        number: 20,
        types: vec![],
        include_list: false,
    })
    .await;

    let err = client::send_binding_request(server_addr, Duration::from_secs(5))
        .await
        .unwrap_err();

    match err {
        client::ClientError::ServerError {
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

#[tokio::test]
async fn test_binding_request_success_with_unknown_optional_attribute() {
    let server_addr = spawn_mock_server(MockResponse::SuccessWithRawAttribute {
        raw_type: 0x80FF,
        value: vec![0xDE, 0xAD, 0xBE, 0xEF],
    })
    .await;

    let result = client::send_binding_request(server_addr, Duration::from_secs(5))
        .await
        .unwrap();

    assert!(result.mapped_address.port() > 0);
}

#[tokio::test]
async fn test_binding_request_success_with_unknown_required_attribute_is_discarded() {
    let server_addr = spawn_mock_server(MockResponse::SuccessWithRawAttribute {
        raw_type: 0x0024,
        value: vec![0x00, 0x00, 0x00, 0x00],
    })
    .await;

    let err = client::send_binding_request(server_addr, Duration::from_millis(150))
        .await
        .unwrap_err();

    assert!(matches!(err, client::ClientError::Timeout(_)));
}

#[tokio::test]
async fn test_binding_request_wrong_transaction_id_is_discarded() {
    let server_addr = spawn_mock_server(MockResponse::WrongTransactionId).await;

    let err = client::send_binding_request(server_addr, Duration::from_millis(150))
        .await
        .unwrap_err();

    assert!(matches!(err, client::ClientError::Timeout(_)));
}

#[tokio::test]
async fn test_binding_request_malformed_response_is_discarded() {
    let server_addr = spawn_mock_server(MockResponse::Malformed).await;

    let err = client::send_binding_request(server_addr, Duration::from_millis(150))
        .await
        .unwrap_err();

    assert!(matches!(err, client::ClientError::Timeout(_)));
}

#[tokio::test]
async fn test_binding_request_bad_xor_length_is_discarded() {
    let server_addr = spawn_mock_server(MockResponse::BadXorLength).await;

    let err = client::send_binding_request(server_addr, Duration::from_millis(150))
        .await
        .unwrap_err();

    assert!(matches!(err, client::ClientError::Timeout(_)));
}

#[tokio::test]
async fn test_binding_request_truncated_xor_address_is_discarded() {
    let server_addr = spawn_mock_server(MockResponse::TruncatedXor).await;

    let err = client::send_binding_request(server_addr, Duration::from_millis(150))
        .await
        .unwrap_err();

    assert!(matches!(err, client::ClientError::Timeout(_)));
}

// ---------------------------------------------------------------------------
// Attributes after MESSAGE-INTEGRITY (RFC 5389 section 15.4)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_xor_mapped_address_after_integrity_is_ignored() {
    let password = "post-mi-secret";
    let key = short_term_key(password).unwrap();
    let (socket, server_addr) = bind_mock().await;
    let handle = tokio::spawn(async move {
        let mut buf = vec![0u8; 65536];
        let (len, from) = socket.recv_from(&mut buf).await.unwrap();
        let request = StunMessage::decode(&buf[..len]).unwrap();

        let mut response = StunMessage::new(StunMessageType::Response, request.transaction_id);
        add_mapped_address(&mut response, from);
        response.add_attribute(StunAttribute::new(
            StunAttributeType::Software,
            b"mock-server".to_vec(),
        ));
        let mut bytes = response.encode_with_integrity(&key);

        let forged = MappedAddress::new("203.0.113.9".parse().unwrap(), 1111);
        let value = forged.encode_xor(request.magic_cookie, &request.transaction_id);
        append_after_integrity(
            &mut bytes,
            StunAttribute::new(StunAttributeType::XorMappedAddress, value),
        );
        socket.send_to(&bytes, from).await.unwrap();
    });

    let auth = client::ClientAuth::short_term("alice", password);
    let result = client::send_binding_request_with_auth(server_addr, Duration::from_secs(2), &auth)
        .await
        .unwrap();

    assert_eq!(result.mapped_address.ip().to_string(), "127.0.0.1");
    assert_ne!(result.mapped_address.port(), 1111);
    handle.await.unwrap();
}

#[tokio::test]
async fn test_xor_mapped_address_only_after_integrity_is_discarded() {
    let password = "post-mi-secret";
    let key = short_term_key(password).unwrap();
    let (socket, server_addr) = bind_mock().await;
    let handle = tokio::spawn(async move {
        let mut buf = vec![0u8; 65536];
        let (len, from) = socket.recv_from(&mut buf).await.unwrap();
        let request = StunMessage::decode(&buf[..len]).unwrap();

        let mut response = StunMessage::new(StunMessageType::Response, request.transaction_id);
        response.add_attribute(StunAttribute::new(
            StunAttributeType::Software,
            b"mock-server".to_vec(),
        ));
        let mut bytes = response.encode_with_integrity(&key);

        let forged = MappedAddress::new("203.0.113.9".parse().unwrap(), 1111);
        let value = forged.encode_xor(request.magic_cookie, &request.transaction_id);
        append_after_integrity(
            &mut bytes,
            StunAttribute::new(StunAttributeType::XorMappedAddress, value),
        );
        socket.send_to(&bytes, from).await.unwrap();
    });

    let auth = client::ClientAuth::short_term("alice", password);
    let err =
        client::send_binding_request_with_auth(server_addr, Duration::from_millis(150), &auth)
            .await
            .unwrap_err();

    assert!(matches!(err, client::ClientError::Timeout(_)));
    handle.await.unwrap();
}

#[tokio::test]
async fn test_unknown_attributes_after_integrity_are_ignored() {
    let password = "post-mi-secret";
    let key = short_term_key(password).unwrap();
    let (socket, server_addr) = bind_mock().await;
    let handle = tokio::spawn(async move {
        let mut buf = vec![0u8; 65536];
        let (len, from) = socket.recv_from(&mut buf).await.unwrap();
        let request = StunMessage::decode(&buf[..len]).unwrap();

        let mut response = StunMessage::new(StunMessageType::ErrorResponse, request.transaction_id);
        let code = stun_protocol::ErrorCode::new(4, 20, "Unknown Attribute".to_string()).unwrap();
        response.add_attribute(StunAttribute::new(
            StunAttributeType::ErrorCode,
            code.encode_value(),
        ));
        response.add_attribute(StunAttribute::new(
            StunAttributeType::Software,
            b"mock-server".to_vec(),
        ));
        let mut bytes = response.encode_with_integrity(&key);

        let list = stun_protocol::UnknownAttributes::new(vec![0x0003]).encode_value();
        append_after_integrity(
            &mut bytes,
            StunAttribute::new(StunAttributeType::UnknownAttributes, list),
        );
        socket.send_to(&bytes, from).await.unwrap();
    });

    let auth = client::ClientAuth::short_term("alice", password);
    let err = client::send_binding_request_with_auth(server_addr, Duration::from_secs(2), &auth)
        .await
        .unwrap_err();

    match err {
        client::ClientError::ServerError {
            class,
            number,
            unknown_attributes,
            ..
        } => {
            assert_eq!((class, number), (4, 20));
            assert!(
                unknown_attributes.is_empty(),
                "attributes after MESSAGE-INTEGRITY must be ignored: {unknown_attributes:?}"
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
    handle.await.unwrap();
}

#[tokio::test]
async fn test_realm_and_nonce_after_integrity_are_ignored() {
    let key = short_term_key("post-mi-secret").unwrap();
    let (socket, server_addr) = bind_mock().await;
    let handle = tokio::spawn(async move {
        let mut buf = vec![0u8; 65536];
        let (len, from) = socket.recv_from(&mut buf).await.unwrap();
        let request = StunMessage::decode(&buf[..len]).unwrap();

        let mut response = StunMessage::new(StunMessageType::ErrorResponse, request.transaction_id);
        let code = stun_protocol::ErrorCode::new(4, 1, "Unauthorized".to_string()).unwrap();
        response.add_attribute(StunAttribute::new(
            StunAttributeType::ErrorCode,
            code.encode_value(),
        ));
        let mut bytes = response.encode_with_integrity(&key);

        for (attribute_type, value) in [
            (StunAttributeType::Realm, b"example.org".to_vec()),
            (StunAttributeType::Nonce, b"nonce-1".to_vec()),
        ] {
            append_after_integrity(&mut bytes, StunAttribute::new(attribute_type, value));
        }
        socket.send_to(&bytes, from).await.unwrap();
    });

    let auth = client::ClientAuth::long_term("alice", "post-mi-secret");
    let err = client::send_binding_request_with_auth(server_addr, Duration::from_secs(2), &auth)
        .await
        .unwrap_err();

    assert!(
        matches!(err, client::ClientError::Protocol(_)),
        "REALM/NONCE after MESSAGE-INTEGRITY must not form a challenge: {err:?}"
    );
    handle.await.unwrap();
}

#[tokio::test]
async fn test_signed_success_with_truncated_duplicate_integrity_is_accepted() {
    let password = "post-mi-secret";
    let key = short_term_key(password).unwrap();
    let (socket, server_addr) = bind_mock().await;
    let handle = tokio::spawn(async move {
        let mut buf = vec![0u8; 65536];
        let (len, from) = socket.recv_from(&mut buf).await.unwrap();
        let request = StunMessage::decode(&buf[..len]).unwrap();

        let mut bytes = signed_success_response(&request, from, &key);
        append_after_integrity(
            &mut bytes,
            StunAttribute::new(StunAttributeType::MessageIntegrity, vec![0xAA; 16]),
        );
        socket.send_to(&bytes, from).await.unwrap();
    });

    let auth = client::ClientAuth::short_term("alice", password);
    let result = client::send_binding_request_with_auth(server_addr, Duration::from_secs(2), &auth)
        .await
        .expect("a truncated duplicate MESSAGE-INTEGRITY must be ignored");

    assert_eq!(result.mapped_address.ip().to_string(), "127.0.0.1");
    handle.await.unwrap();
}

#[tokio::test]
async fn test_error_code_after_integrity_is_ignored() {
    let password = "post-mi-secret";
    let key = short_term_key(password).unwrap();
    let (socket, server_addr) = bind_mock().await;
    let handle = tokio::spawn(async move {
        let mut buf = vec![0u8; 65536];
        let (len, from) = socket.recv_from(&mut buf).await.unwrap();
        let request = StunMessage::decode(&buf[..len]).unwrap();

        let mut response = StunMessage::new(StunMessageType::ErrorResponse, request.transaction_id);
        response.add_attribute(StunAttribute::new(
            StunAttributeType::Software,
            b"mock-server".to_vec(),
        ));
        let mut bytes = response.encode_with_integrity(&key);

        let code = stun_protocol::ErrorCode::new(4, 1, "Unauthorized".to_string()).unwrap();
        append_after_integrity(
            &mut bytes,
            StunAttribute::new(StunAttributeType::ErrorCode, code.encode_value()),
        );
        socket.send_to(&bytes, from).await.unwrap();
    });

    let auth = client::ClientAuth::short_term("alice", password);
    let err =
        client::send_binding_request_with_auth(server_addr, Duration::from_millis(150), &auth)
            .await
            .unwrap_err();

    assert!(
        matches!(err, client::ClientError::Timeout(_)),
        "ERROR-CODE after MESSAGE-INTEGRITY must be ignored: {err:?}"
    );
    handle.await.unwrap();
}

// ---------------------------------------------------------------------------
// Short-term authentication (FR-040, FR-042)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_short_term_request_is_signed_and_response_validated() {
    let password = "s3cret-short-term";
    let key = short_term_key(password).unwrap();
    let (socket, server_addr) = bind_mock().await;
    let server_key = key.clone();
    let handle = tokio::spawn(async move {
        let mut buf = vec![0u8; 65536];
        let (len, from) = socket.recv_from(&mut buf).await.unwrap();
        let request = StunMessage::decode(&buf[..len]).unwrap();

        assert_eq!(
            text_attribute(&request, StunAttributeType::Username),
            Some("alice")
        );
        assert_eq!(
            request.attribute_count(StunAttributeType::MessageIntegrity as u16),
            1
        );
        assert_eq!(
            text_attribute(&request, StunAttributeType::Software),
            Some(client::SOFTWARE)
        );
        assert!(StunMessage::verify_message_integrity(
            &buf[..len],
            &server_key
        ));

        let response = signed_success_response(&request, from, &server_key);
        socket.send_to(&response, from).await.unwrap();
    });

    let auth = client::ClientAuth::short_term("alice", password);
    let result = client::send_binding_request_with_auth(server_addr, Duration::from_secs(2), &auth)
        .await
        .unwrap();

    assert!(result.mapped_address.port() > 0);
    handle.await.unwrap();
}

#[tokio::test]
async fn test_short_term_response_without_integrity_is_discarded() {
    let password = "s3cret-short-term";
    let (socket, server_addr) = bind_mock().await;
    let handle = tokio::spawn(async move {
        let mut buf = vec![0u8; 65536];
        let (len, from) = socket.recv_from(&mut buf).await.unwrap();
        let request = StunMessage::decode(&buf[..len]).unwrap();
        let response = success_response(&request, from, false);
        socket.send_to(&response, from).await.unwrap();
    });

    let auth = client::ClientAuth::short_term("alice", password);
    let err =
        client::send_binding_request_with_auth(server_addr, Duration::from_millis(150), &auth)
            .await
            .unwrap_err();

    assert!(matches!(err, client::ClientError::Timeout(_)));
    handle.await.unwrap();
}

#[tokio::test]
async fn test_short_term_response_with_wrong_integrity_is_discarded() {
    let password = "s3cret-short-term";
    let (socket, server_addr) = bind_mock().await;
    let handle = tokio::spawn(async move {
        let mut buf = vec![0u8; 65536];
        let (len, from) = socket.recv_from(&mut buf).await.unwrap();
        let request = StunMessage::decode(&buf[..len]).unwrap();
        let response = signed_success_response(&request, from, b"wrong-key");
        socket.send_to(&response, from).await.unwrap();
    });

    let auth = client::ClientAuth::short_term("alice", password);
    let err =
        client::send_binding_request_with_auth(server_addr, Duration::from_millis(150), &auth)
            .await
            .unwrap_err();

    assert!(matches!(err, client::ClientError::Timeout(_)));
    handle.await.unwrap();
}

#[tokio::test]
async fn test_short_term_401_is_terminal() {
    let (socket, server_addr) = bind_mock().await;
    let requests = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let observed = requests.clone();
    let handle = tokio::spawn(async move {
        let mut buf = vec![0u8; 65536];
        for _ in 0..1 {
            let (len, from) = socket.recv_from(&mut buf).await.unwrap();
            let request = StunMessage::decode(&buf[..len]).unwrap();
            observed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let response = challenge_response(
                request.transaction_id,
                4,
                1,
                "Unauthorized",
                Some(("example.org", "nonce-1")),
            );
            socket.send_to(&response, from).await.unwrap();
        }
    });

    let auth = client::ClientAuth::short_term("alice", "s3cret-short-term");
    let err = client::send_binding_request_with_auth(server_addr, Duration::from_secs(2), &auth)
        .await
        .unwrap_err();

    match err {
        client::ClientError::ServerError { class, number, .. } => {
            assert_eq!((class, number), (4, 1));
        }
        other => panic!("unexpected error: {:?}", other),
    }
    handle.await.unwrap();
    assert_eq!(requests.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_short_term_400_without_integrity_is_accepted() {
    // RFC 5389 section 15.6: the server may be unable to compute a valid
    // MESSAGE-INTEGRITY for a 400, so the client must not expect one.
    let (socket, server_addr) = bind_mock().await;
    let handle = tokio::spawn(async move {
        let mut buf = vec![0u8; 65536];
        let (len, from) = socket.recv_from(&mut buf).await.unwrap();
        let request = StunMessage::decode(&buf[..len]).unwrap();
        let response = challenge_response(request.transaction_id, 4, 0, "Bad Request", None);
        socket.send_to(&response, from).await.unwrap();
    });

    let auth = client::ClientAuth::short_term("alice", "s3cret-short-term");
    let err = client::send_binding_request_with_auth(server_addr, Duration::from_secs(2), &auth)
        .await
        .unwrap_err();

    match err {
        client::ClientError::ServerError { class, number, .. } => {
            assert_eq!((class, number), (4, 0));
        }
        other => panic!("unexpected error: {:?}", other),
    }
    handle.await.unwrap();
}

// ---------------------------------------------------------------------------
// Long-term authentication (FR-041, SC-011)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_long_term_401_challenge_retries_once() {
    let password = "s3cret-long-term";
    let (socket, server_addr) = bind_mock().await;
    let requests = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let observed = requests.clone();
    let handle = tokio::spawn(async move {
        let mut buf = vec![0u8; 65536];
        for expected in 0..2 {
            let (len, from) = socket.recv_from(&mut buf).await.unwrap();
            let request = StunMessage::decode(&buf[..len]).unwrap();
            observed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

            if expected == 0 {
                assert_eq!(
                    request.attribute_count(StunAttributeType::MessageIntegrity as u16),
                    0,
                    "initial long-term request must be unauthenticated"
                );
                let response = challenge_response(
                    request.transaction_id,
                    4,
                    1,
                    "Unauthorized",
                    Some(("example.org", "nonce-1")),
                );
                socket.send_to(&response, from).await.unwrap();
            } else {
                let key = long_term_key("alice", "example.org", password).unwrap();
                assert_eq!(
                    text_attribute(&request, StunAttributeType::Username),
                    Some("alice")
                );
                assert_eq!(
                    text_attribute(&request, StunAttributeType::Realm),
                    Some("example.org")
                );
                assert_eq!(
                    text_attribute(&request, StunAttributeType::Nonce),
                    Some("nonce-1")
                );
                assert!(StunMessage::verify_message_integrity(&buf[..len], &key));
                let response = signed_success_response(&request, from, &key);
                socket.send_to(&response, from).await.unwrap();
            }
        }
    });

    let auth = client::ClientAuth::long_term("alice", password);
    let result = client::send_binding_request_with_auth(server_addr, Duration::from_secs(2), &auth)
        .await
        .unwrap();

    assert!(result.mapped_address.port() > 0);
    handle.await.unwrap();
    assert_eq!(requests.load(std::sync::atomic::Ordering::SeqCst), 2);
}

#[tokio::test]
async fn test_long_term_realm_override_replaces_challenge_realm() {
    let password = "s3cret-long-term";
    let (socket, server_addr) = bind_mock().await;
    let handle = tokio::spawn(async move {
        let mut buf = vec![0u8; 65536];
        for expected in 0..2 {
            let (len, from) = socket.recv_from(&mut buf).await.unwrap();
            let request = StunMessage::decode(&buf[..len]).unwrap();

            if expected == 0 {
                let response = challenge_response(
                    request.transaction_id,
                    4,
                    1,
                    "Unauthorized",
                    Some(("server.example", "nonce-1")),
                );
                socket.send_to(&response, from).await.unwrap();
            } else {
                assert_eq!(
                    text_attribute(&request, StunAttributeType::Realm),
                    Some("exämple.org"),
                    "the override must be sent SASLprep-prepared instead of the challenge realm"
                );
                assert_eq!(
                    text_attribute(&request, StunAttributeType::Nonce),
                    Some("nonce-1"),
                    "the challenge NONCE must still be used"
                );
                let key = long_term_key("alice", "exämple.org", password).unwrap();
                assert!(StunMessage::verify_message_integrity(&buf[..len], &key));
                let response = signed_success_response(&request, from, &key);
                socket.send_to(&response, from).await.unwrap();
            }
        }
    });

    let auth = client::ClientAuth::long_term_with_realm("alice", password, "exa\u{0308}mple.org");
    let result = client::send_binding_request_with_auth(server_addr, Duration::from_secs(2), &auth)
        .await
        .unwrap();

    assert!(result.mapped_address.port() > 0);
    handle.await.unwrap();
}

#[tokio::test]
async fn test_long_term_438_adopts_fresh_nonce_within_retry_bound() {
    let password = "s3cret-long-term";
    let (socket, server_addr) = bind_mock().await;
    let requests = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let observed = requests.clone();
    let handle = tokio::spawn(async move {
        let mut buf = vec![0u8; 65536];
        for expected in 0..3 {
            let (len, from) = socket.recv_from(&mut buf).await.unwrap();
            let request = StunMessage::decode(&buf[..len]).unwrap();
            observed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let key = long_term_key("alice", "example.org", password).unwrap();

            match expected {
                0 => {
                    let response = challenge_response(
                        request.transaction_id,
                        4,
                        1,
                        "Unauthorized",
                        Some(("example.org", "nonce-1")),
                    );
                    socket.send_to(&response, from).await.unwrap();
                }
                1 => {
                    assert_eq!(
                        text_attribute(&request, StunAttributeType::Nonce),
                        Some("nonce-1")
                    );
                    assert!(StunMessage::verify_message_integrity(&buf[..len], &key));
                    let response = challenge_response(
                        request.transaction_id,
                        4,
                        38,
                        "Stale Nonce",
                        Some(("example.org", "nonce-2")),
                    );
                    socket.send_to(&response, from).await.unwrap();
                }
                _ => {
                    assert_eq!(
                        text_attribute(&request, StunAttributeType::Nonce),
                        Some("nonce-2"),
                        "client must adopt the fresh NONCE"
                    );
                    assert!(StunMessage::verify_message_integrity(&buf[..len], &key));
                    let response = signed_success_response(&request, from, &key);
                    socket.send_to(&response, from).await.unwrap();
                }
            }
        }
    });

    let auth = client::ClientAuth::long_term("alice", password);
    let result = client::send_binding_request_with_auth(server_addr, Duration::from_secs(2), &auth)
        .await
        .unwrap();

    assert!(result.mapped_address.port() > 0);
    handle.await.unwrap();
    assert_eq!(requests.load(std::sync::atomic::Ordering::SeqCst), 3);
}

// ---------------------------------------------------------------------------
// Challenge MESSAGE-INTEGRITY (RFC 5389 sections 10.1.3 and 10.2.3)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_long_term_438_with_invalid_integrity_is_discarded() {
    let password = "s3cret-long-term";
    let (socket, server_addr) = bind_mock().await;
    let handle = tokio::spawn(async move {
        let mut buf = vec![0u8; 65536];
        let mut request_count = 0;
        loop {
            let recv =
                tokio::time::timeout(Duration::from_millis(500), socket.recv_from(&mut buf)).await;
            let Ok(Ok((len, from))) = recv else {
                break;
            };
            request_count += 1;
            let request = StunMessage::decode(&buf[..len]).unwrap();

            let response = match request_count {
                1 => challenge_response(
                    request.transaction_id,
                    4,
                    1,
                    "Unauthorized",
                    Some(("example.org", "nonce-1")),
                ),
                2 => signed_challenge_response(
                    request.transaction_id,
                    4,
                    38,
                    "Stale Nonce",
                    Some(("example.org", "nonce-2")),
                    b"wrong-key-material",
                ),
                _ => {
                    let key = long_term_key("alice", "example.org", password).unwrap();
                    signed_success_response(&request, from, &key)
                }
            };
            socket.send_to(&response, from).await.unwrap();
            if request_count >= 3 {
                break;
            }
        }
    });

    let auth = client::ClientAuth::long_term("alice", password);
    let err =
        client::send_binding_request_with_auth(server_addr, Duration::from_millis(200), &auth)
            .await
            .unwrap_err();

    assert!(
        matches!(err, client::ClientError::Timeout(_)),
        "a challenge with invalid MESSAGE-INTEGRITY must be discarded: {err:?}"
    );
    handle.await.unwrap();
}

#[tokio::test]
async fn test_long_term_438_with_valid_integrity_is_accepted() {
    let password = "s3cret-long-term";
    let key = long_term_key("alice", "example.org", password).unwrap();
    let (socket, server_addr) = bind_mock().await;
    let handle = tokio::spawn(async move {
        let mut buf = vec![0u8; 65536];
        for expected in 0..3 {
            let (len, from) = socket.recv_from(&mut buf).await.unwrap();
            let request = StunMessage::decode(&buf[..len]).unwrap();

            let response = match expected {
                0 => challenge_response(
                    request.transaction_id,
                    4,
                    1,
                    "Unauthorized",
                    Some(("example.org", "nonce-1")),
                ),
                1 => signed_challenge_response(
                    request.transaction_id,
                    4,
                    38,
                    "Stale Nonce",
                    Some(("example.org", "nonce-2")),
                    &key,
                ),
                _ => {
                    assert_eq!(
                        text_attribute(&request, StunAttributeType::Nonce),
                        Some("nonce-2"),
                        "client must adopt the fresh NONCE from a signed challenge"
                    );
                    signed_success_response(&request, from, &key)
                }
            };
            socket.send_to(&response, from).await.unwrap();
        }
    });

    let auth = client::ClientAuth::long_term("alice", password);
    let result = client::send_binding_request_with_auth(server_addr, Duration::from_secs(2), &auth)
        .await
        .unwrap();

    assert_eq!(result.mapped_address.ip().to_string(), "127.0.0.1");
    handle.await.unwrap();
}

#[tokio::test]
async fn test_short_term_400_with_invalid_integrity_is_discarded() {
    let password = "post-mi-secret";
    let (socket, server_addr) = bind_mock().await;
    let handle = tokio::spawn(async move {
        let mut buf = vec![0u8; 65536];
        let (len, from) = socket.recv_from(&mut buf).await.unwrap();
        let request = StunMessage::decode(&buf[..len]).unwrap();
        let response = signed_challenge_response(
            request.transaction_id,
            4,
            0,
            "Bad Request",
            None,
            b"wrong-key-material",
        );
        socket.send_to(&response, from).await.unwrap();
    });

    let auth = client::ClientAuth::short_term("alice", password);
    let err =
        client::send_binding_request_with_auth(server_addr, Duration::from_millis(150), &auth)
            .await
            .unwrap_err();

    assert!(
        matches!(err, client::ClientError::Timeout(_)),
        "a 400 carrying an invalid MESSAGE-INTEGRITY must be discarded: {err:?}"
    );
    handle.await.unwrap();
}

#[tokio::test]
async fn test_long_term_401_after_signed_request_is_terminal() {
    let (socket, server_addr) = bind_mock().await;
    let requests = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let observed = requests.clone();
    let handle = tokio::spawn(async move {
        let mut buf = vec![0u8; 65536];
        for _ in 0..2 {
            let (len, from) = socket.recv_from(&mut buf).await.unwrap();
            let request = StunMessage::decode(&buf[..len]).unwrap();
            observed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let response = challenge_response(
                request.transaction_id,
                4,
                1,
                "Unauthorized",
                Some(("example.org", "nonce-1")),
            );
            socket.send_to(&response, from).await.unwrap();
        }
    });

    let auth = client::ClientAuth::long_term("alice", "s3cret-long-term");
    let err = client::send_binding_request_with_auth(server_addr, Duration::from_secs(2), &auth)
        .await
        .unwrap_err();

    match err {
        client::ClientError::ServerError { class, number, .. } => {
            assert_eq!((class, number), (4, 1));
        }
        other => panic!("unexpected error: {:?}", other),
    }
    handle.await.unwrap();
    assert_eq!(
        requests.load(std::sync::atomic::Ordering::SeqCst),
        2,
        "a 401 after a signed request must not be retried without changing credentials"
    );
}

#[tokio::test]
async fn test_error_response_with_unknown_required_attribute_is_discarded() {
    let password = "s3cret-long-term";
    let key = long_term_key("alice", "example.org", password).unwrap();
    let (socket, server_addr) = bind_mock().await;
    let handle = tokio::spawn(async move {
        let mut buf = vec![0u8; 65536];
        loop {
            let Ok(Ok((len, from))) =
                tokio::time::timeout(Duration::from_millis(300), socket.recv_from(&mut buf)).await
            else {
                break;
            };
            let request = StunMessage::decode(&buf[..len]).unwrap();

            let mut response =
                StunMessage::new(StunMessageType::ErrorResponse, request.transaction_id);
            let code = stun_protocol::ErrorCode::new(4, 1, "Unauthorized".to_string()).unwrap();
            response.add_attribute(StunAttribute::new(
                StunAttributeType::ErrorCode,
                code.encode_value(),
            ));
            response.add_attribute(StunAttribute::new(
                StunAttributeType::Realm,
                b"example.org".to_vec(),
            ));
            response.add_attribute(StunAttribute::new(
                StunAttributeType::Nonce,
                b"nonce-1".to_vec(),
            ));
            response.add_attribute(raw_attribute(0x0003, &[0u8; 4]));
            socket
                .send_to(&response.encode_with_integrity(&key), from)
                .await
                .unwrap();
        }
    });

    let auth = client::ClientAuth::long_term("alice", password);
    let err =
        client::send_binding_request_with_auth(server_addr, Duration::from_millis(150), &auth)
            .await
            .unwrap_err();

    assert!(
        matches!(err, client::ClientError::Timeout(_)),
        "an error response with unknown comprehension-required attributes must be discarded: {err:?}"
    );
    handle.await.unwrap();
}

#[tokio::test]
async fn test_long_term_challenge_without_realm_or_nonce_is_protocol_error() {
    let (socket, server_addr) = bind_mock().await;
    let handle = tokio::spawn(async move {
        let mut buf = vec![0u8; 65536];
        let (len, from) = socket.recv_from(&mut buf).await.unwrap();
        let request = StunMessage::decode(&buf[..len]).unwrap();
        let response = challenge_response(request.transaction_id, 4, 1, "Unauthorized", None);
        socket.send_to(&response, from).await.unwrap();
    });

    let auth = client::ClientAuth::long_term("alice", "s3cret-long-term");
    let err = client::send_binding_request_with_auth(server_addr, Duration::from_secs(2), &auth)
        .await
        .unwrap_err();

    assert!(matches!(err, client::ClientError::Protocol(_)));
    handle.await.unwrap();
}

#[tokio::test]
async fn test_disabled_client_reports_challenge_without_retry() {
    let (socket, server_addr) = bind_mock().await;
    let requests = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let observed = requests.clone();
    let handle = tokio::spawn(async move {
        let mut buf = vec![0u8; 65536];
        let (len, from) = socket.recv_from(&mut buf).await.unwrap();
        let request = StunMessage::decode(&buf[..len]).unwrap();
        observed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let response = challenge_response(
            request.transaction_id,
            4,
            38,
            "Stale Nonce",
            Some(("example.org", "nonce-1")),
        );
        socket.send_to(&response, from).await.unwrap();
    });

    let err = client::send_binding_request(server_addr, Duration::from_secs(2))
        .await
        .unwrap_err();

    match err {
        client::ClientError::ServerError {
            class,
            number,
            reason,
            ..
        } => {
            assert_eq!((class, number), (4, 38));
            assert_eq!(reason, "Stale Nonce");
        }
        other => panic!("unexpected error: {:?}", other),
    }
    handle.await.unwrap();
    assert_eq!(requests.load(std::sync::atomic::Ordering::SeqCst), 1);
}

// ---------------------------------------------------------------------------
// Secret redaction (SC-010)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_authentication_errors_do_not_leak_secrets() {
    let password = "top-secret-password";
    let server_addr = spawn_mock_server(MockResponse::Malformed).await;

    let auth = client::ClientAuth::short_term("alice", password);
    let err = client::send_binding_request_with_auth(server_addr, Duration::from_secs(2), &auth)
        .await
        .unwrap_err();

    for rendered in [err.to_string(), format!("{:?}", err)] {
        assert!(!rendered.contains(password), "secret leaked: {rendered}");
    }
    let debug = format!("{:?}", auth);
    assert!(!debug.contains(password), "secret leaked: {debug}");
    assert!(!debug.contains(&hex_encode(password.as_bytes())));
}

#[tokio::test]
async fn test_debug_logging_redacts_credentials() {
    install_capture_logger();
    CAPTURED.lock().unwrap().clear();

    let password = "log-redaction-secret";
    let key = short_term_key(password).unwrap();
    let (socket, server_addr) = bind_mock().await;
    let server_key = key.clone();
    let handle = tokio::spawn(async move {
        let mut buf = vec![0u8; 65536];
        let (len, from) = socket.recv_from(&mut buf).await.unwrap();
        let request = StunMessage::decode(&buf[..len]).unwrap();
        let response = signed_success_response(&request, from, &server_key);
        socket.send_to(&response, from).await.unwrap();
    });

    let auth = client::ClientAuth::short_term("alice", password);
    client::send_binding_request_with_auth(server_addr, Duration::from_secs(2), &auth)
        .await
        .unwrap();
    handle.await.unwrap();

    let captured = CAPTURED.lock().unwrap().clone();
    assert!(
        captured
            .iter()
            .any(|line| line.contains("auth=short-term") && line.contains("user=alice")),
        "expected auth log line, captured: {:?}",
        captured
    );
    let key_hex = hex_encode(&key);
    for line in &captured {
        assert!(!line.contains(password), "password leaked: {line}");
        assert!(!line.contains(&key_hex), "derived key leaked: {line}");
    }
}
