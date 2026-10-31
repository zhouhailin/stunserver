//! Integration tests for STUN Server.
//!
//! Tests cover configuration loading from files, STUN request/response
//! over a real UDP loopback, and error handling for invalid configurations.

use std::io::Write;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use stun_protocol::integrity::{long_term_key, short_term_key};
use stun_protocol::{
    ErrorCode, MappedAddress, StunAttribute, StunAttributeType, StunMessage, StunMessageType,
    UnknownAttributes,
};
use stunserver::auth::AuthService;
use stunserver::config;
use stunserver::server;
use tempfile::NamedTempFile;

/// Helper: write TOML to a temp file and return its path.
fn write_temp_config(toml_str: &str) -> NamedTempFile {
    let mut f = NamedTempFile::new().expect("create temp file");
    f.write_all(toml_str.as_bytes()).expect("write temp config");
    f
}

// -----------------------------------------------------------------------
// Configuration loading tests
// -----------------------------------------------------------------------

#[test]
fn test_load_config_from_file() {
    let toml_str = r#"
[server]
listen_address = "127.0.0.1"
port = 13478

[logging]
level = "debug"
enable_console = true
"#;
    let f = write_temp_config(toml_str);
    let cfg = config::load_config(f.path()).unwrap();
    assert_eq!(cfg.server.listen_address, "127.0.0.1");
    assert_eq!(cfg.server.port, 13478);
    assert_eq!(cfg.logging.level, "debug");
    assert!(cfg.logging.enable_console);
}

#[test]
fn test_load_config_file_not_found() {
    let result = config::load_config(std::path::Path::new("/nonexistent/stunserver.toml"));
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("not found"));
}

#[test]
fn test_load_config_invalid_toml() {
    let toml_str = "{{invalid toml::";
    let f = write_temp_config(toml_str);
    let result = config::load_config(f.path());
    assert!(result.is_err());
}

#[test]
fn test_load_config_invalid_log_level() {
    let toml_str = r#"
[logging]
level = "verbose"
"#;
    let f = write_temp_config(toml_str);
    let result = config::load_config(f.path());
    assert!(result.is_err());
}

#[test]
fn test_load_config_with_file_logging() {
    let toml_str = r#"
[server]
listen_address = "0.0.0.0"
port = 3478

[logging]
level = "info"
log_dir = "/tmp/stunserver-test"
file_prefix = "test"
max_file_size_mb = 50
retention_days = 7
enable_console = false
"#;
    let f = write_temp_config(toml_str);
    let cfg = config::load_config(f.path()).unwrap();
    assert_eq!(
        cfg.logging.log_dir.to_str().unwrap(),
        "/tmp/stunserver-test"
    );
    assert_eq!(cfg.logging.file_prefix, "test");
    assert_eq!(cfg.logging.max_file_size_mb, 50);
    assert_eq!(cfg.logging.retention_days, 7);
    assert!(!cfg.logging.enable_console);
}

#[test]
fn test_default_config_values() {
    let cfg = config::default_config();
    assert_eq!(cfg.server.listen_address, "0.0.0.0");
    assert_eq!(cfg.server.port, 3478);
    assert_eq!(cfg.logging.level, "info");
}

#[test]
fn test_load_config_with_network_section() {
    let toml_str = r#"
[network]
udp_recv_buffer_size = 2097152
udp_send_buffer_size = 2097152
tcp_connection_timeout_sec = 60
tcp_idle_timeout_sec = 600
"#;
    let f = write_temp_config(toml_str);
    let cfg = config::load_config(f.path()).unwrap();
    assert_eq!(cfg.network.udp_recv_buffer_size, 2097152);
    assert_eq!(cfg.network.udp_send_buffer_size, 2097152);
    assert_eq!(cfg.network.tcp_connection_timeout_sec, 60);
    assert_eq!(cfg.network.tcp_idle_timeout_sec, 600);
}

#[test]
fn test_load_config_with_security_section() {
    let toml_str = r#"
[security]
enable_tls = false
"#;
    let f = write_temp_config(toml_str);
    let cfg = config::load_config(f.path()).unwrap();
    assert!(!cfg.security.enable_tls);
}

#[test]
fn test_validate_invalid_port() {
    let toml_str = r#"
[server]
port = 0
"#;
    let f = write_temp_config(toml_str);
    let result = config::load_config(f.path());
    assert!(result.is_err());
}

#[test]
fn test_validate_invalid_worker_threads() {
    let toml_str = r#"
[server]
worker_threads = 300
"#;
    let f = write_temp_config(toml_str);
    let result = config::load_config(f.path());
    assert!(result.is_err());
}

// -----------------------------------------------------------------------
// STUN request/response integration test (real UDP loopback)
// -----------------------------------------------------------------------

#[tokio::test]
async fn test_stun_binding_request_response() {
    // Bind a server socket on a random port.
    let server_socket = tokio::net::UdpSocket::bind("127.0.0.1:0")
        .await
        .expect("bind server socket");
    let server_addr = server_socket.local_addr().unwrap();
    let server_socket = Arc::new(server_socket);

    // Spawn a simple handler that uses build_response from the server module.
    let sock = server_socket.clone();
    tokio::spawn(async move {
        let mut buf = vec![0u8; 65536];
        // Handle one request then exit.
        let (len, client_addr) = sock.recv_from(&mut buf).await.unwrap();
        let response = server::build_response(&buf[..len], client_addr).unwrap();
        sock.send_to(&response, client_addr).await.unwrap();
    });

    // Create a client socket.
    let client_socket = tokio::net::UdpSocket::bind("127.0.0.1:0")
        .await
        .expect("bind client socket");

    // Build a STUN Binding Request.
    let transaction_id = [0x42; 12];
    let request = StunMessage::new(StunMessageType::Request, transaction_id);
    let request_bytes = request.encode();

    // Send request to server.
    client_socket
        .send_to(&request_bytes, server_addr)
        .await
        .unwrap();

    // Receive response.
    let mut resp_buf = vec![0u8; 65536];
    let (resp_len, _from) = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        client_socket.recv_from(&mut resp_buf),
    )
    .await
    .expect("timeout waiting for response")
    .expect("recv response");

    // Parse and verify response.
    let response = StunMessage::decode(&resp_buf[..resp_len]).unwrap();
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

    // Verify the mapped address matches the client's address.
    let client_addr = client_socket.local_addr().unwrap();
    let mapped = MappedAddress::decode_xor(
        &response.attributes[0].attribute_value,
        response.magic_cookie,
        &response.transaction_id,
    )
    .unwrap();
    assert_eq!(mapped.address, client_addr.ip());
    assert_eq!(mapped.port, client_addr.port());
}

// -----------------------------------------------------------------------
// 420 Unknown Attribute over real UDP loopback
// -----------------------------------------------------------------------

/// Build a StunAttribute from a raw type code and value.
fn raw_attribute(raw_type: u16, value: &[u8]) -> stun_protocol::StunAttribute {
    let mut bytes = Vec::with_capacity(4 + value.len());
    bytes.extend_from_slice(&raw_type.to_be_bytes());
    bytes.extend_from_slice(&(value.len() as u16).to_be_bytes());
    bytes.extend_from_slice(value);
    stun_protocol::StunAttribute::decode(&bytes).expect("raw attribute")
}

/// Send one Binding Request carrying `attributes` to a loopback server and
/// return the decoded response.
async fn exchange(attributes: Vec<stun_protocol::StunAttribute>) -> stun_protocol::StunMessage {
    use std::time::Duration;

    let server_socket = tokio::net::UdpSocket::bind("127.0.0.1:0")
        .await
        .expect("bind server socket");
    let server_addr = server_socket.local_addr().unwrap();
    let server_socket = Arc::new(server_socket);

    let sock = server_socket.clone();
    tokio::spawn(async move {
        let mut buf = vec![0u8; 65536];
        let (len, client_addr) = sock.recv_from(&mut buf).await.unwrap();
        let response = server::build_response(&buf[..len], client_addr).unwrap();
        sock.send_to(&response, client_addr).await.unwrap();
    });

    let client_socket = tokio::net::UdpSocket::bind("127.0.0.1:0")
        .await
        .expect("bind client socket");

    let transaction_id = [0x5A; 12];
    let mut request = StunMessage::new(StunMessageType::Request, transaction_id);
    for attr in attributes {
        request.add_attribute(attr);
    }

    client_socket
        .send_to(&request.encode(), server_addr)
        .await
        .unwrap();

    let mut resp_buf = vec![0u8; 65536];
    let (resp_len, _from) = tokio::time::timeout(
        Duration::from_secs(2),
        client_socket.recv_from(&mut resp_buf),
    )
    .await
    .expect("timeout waiting for response")
    .expect("recv response");

    let response = StunMessage::decode(&resp_buf[..resp_len]).unwrap();
    assert_eq!(response.transaction_id, transaction_id);
    response
}

#[tokio::test]
async fn test_unknown_required_attribute_gets_420() {
    let response = exchange(vec![raw_attribute(0x0024, &[0x00, 0x00, 0x00, 0x01])]).await;

    assert_eq!(response.message_type, StunMessageType::ErrorIndication);
    assert_eq!(response.attributes.len(), 3);
    assert_eq!(
        response.attributes[2].attribute_type,
        StunAttributeType::Software
    );

    let code = stun_protocol::ErrorCode::decode_value(&response.attributes[0].attribute_value)
        .expect("ERROR-CODE decodes");
    assert_eq!(code.status_code(), 420);
    assert_eq!(code.reason, "Unknown Attribute");

    let unknown =
        stun_protocol::UnknownAttributes::decode_value(&response.attributes[1].attribute_value)
            .expect("UNKNOWN-ATTRIBUTES decodes");
    assert_eq!(unknown.types, vec![0x0024]);

    assert!(response
        .attributes
        .iter()
        .all(|attr| attr.attribute_type != StunAttributeType::XorMappedAddress));
}

#[tokio::test]
async fn test_duplicate_unknown_required_attributes_deduplicated() {
    let response = exchange(vec![
        raw_attribute(0x0024, &[0x00, 0x00, 0x00, 0x00]),
        raw_attribute(0x0025, &[0x00, 0x00, 0x00, 0x00]),
        raw_attribute(0x0024, &[0x00, 0x00, 0x00, 0x00]),
    ])
    .await;

    assert_eq!(response.message_type, StunMessageType::ErrorIndication);
    let unknown =
        stun_protocol::UnknownAttributes::decode_value(&response.attributes[1].attribute_value)
            .unwrap();
    assert_eq!(unknown.types, vec![0x0024, 0x0025]);
}

#[tokio::test]
async fn test_unknown_optional_only_gets_success() {
    let response = exchange(vec![raw_attribute(0x80FF, &[0xDE, 0xAD, 0xBE, 0xEF])]).await;

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

#[tokio::test]
async fn test_rfc5780_unsupported_attributes_get_420() {
    for raw_type in [0x0003u16, 0x0026, 0x0027] {
        let response = exchange(vec![raw_attribute(raw_type, &[0x00, 0x00, 0x00, 0x00])]).await;

        assert_eq!(
            response.message_type,
            StunMessageType::ErrorIndication,
            "attribute 0x{raw_type:04X} must be rejected"
        );

        let code = stun_protocol::ErrorCode::decode_value(&response.attributes[0].attribute_value)
            .expect("ERROR-CODE decodes");
        assert_eq!(code.status_code(), 420);

        let unknown =
            stun_protocol::UnknownAttributes::decode_value(&response.attributes[1].attribute_value)
                .expect("UNKNOWN-ATTRIBUTES decodes");
        assert_eq!(unknown.types, vec![raw_type]);
    }
}

#[tokio::test]
async fn test_known_required_attributes_are_not_listed() {
    // MAPPED-ADDRESS, ERROR-CODE, UNKNOWN-ATTRIBUTES, and ALTERNATE-SERVER are
    // known attributes the server does not act on; they must not cause 420.
    let response = exchange(vec![
        raw_attribute(0x0001, &[0x00, 0x01, 0x00, 0x00, 0x7F, 0x00, 0x00, 0x01]),
        raw_attribute(0x0009, &[0x00, 0x00, 0x04, 0x00]),
        raw_attribute(0x000A, &[0x00, 0x24, 0x00, 0x00]),
        raw_attribute(0x8023, &[0x00, 0x01, 0x00, 0x00, 0x7F, 0x00, 0x00, 0x01]),
    ])
    .await;

    assert_eq!(response.message_type, StunMessageType::Response);
    assert_eq!(
        response.attributes[0].attribute_type,
        StunAttributeType::XorMappedAddress
    );
}

#[tokio::test]
async fn test_mixed_request_lists_only_unknown_required_types() {
    let response = exchange(vec![
        raw_attribute(0x8001, &[0xDE, 0xAD, 0xBE, 0xEF]),
        raw_attribute(0x0024, &[0x00, 0x00, 0x00, 0x01]),
        raw_attribute(0x0025, &[0x00, 0x00, 0x00, 0x00]),
    ])
    .await;

    assert_eq!(response.message_type, StunMessageType::ErrorIndication);
    let unknown = UnknownAttributes::decode_value(&response.attributes[1].attribute_value).unwrap();
    assert_eq!(unknown.types, vec![0x0024, 0x0025]);
}

#[tokio::test]
async fn test_duplicate_unknown_types_listed_once_in_first_seen_order() {
    let response = exchange(vec![
        raw_attribute(0x0025, &[0x00, 0x00, 0x00, 0x00]),
        raw_attribute(0x0024, &[0x00, 0x00, 0x00, 0x01]),
        raw_attribute(0x0025, &[0x00, 0x00, 0x00, 0x00]),
        raw_attribute(0x0024, &[0x00, 0x00, 0x00, 0x01]),
    ])
    .await;

    let unknown = UnknownAttributes::decode_value(&response.attributes[1].attribute_value).unwrap();
    assert_eq!(unknown.types, vec![0x0025, 0x0024]);
}

#[tokio::test]
async fn test_unknown_optional_attributes_ignored() {
    let response = exchange(vec![
        raw_attribute(0x8001, &[0x01, 0x02, 0x03, 0x04]),
        raw_attribute(0xBEEF, &[0x05, 0x06, 0x07, 0x08]),
    ])
    .await;

    assert_eq!(response.message_type, StunMessageType::Response);
}

#[tokio::test]
async fn test_auth_attributes_ignored_when_disabled() {
    // USERNAME, MESSAGE-INTEGRITY, REALM, and NONCE are recognized protocol
    // attributes; with authentication disabled they must not trigger 420.
    let response = exchange(vec![
        StunAttribute::new(StunAttributeType::Username, b"alice".to_vec()),
        StunAttribute::new(StunAttributeType::MessageIntegrity, vec![0u8; 20]),
        StunAttribute::new(StunAttributeType::Realm, b"example.org".to_vec()),
        StunAttribute::new(StunAttributeType::Nonce, vec![b'a'; 48]),
    ])
    .await;

    assert_eq!(response.message_type, StunMessageType::Response);
    assert!(response
        .attributes
        .iter()
        .all(|attr| attr.attribute_type != StunAttributeType::UnknownAttributes));
}

#[tokio::test]
async fn test_responses_never_contain_other_address() {
    let success = exchange(vec![]).await;
    assert_eq!(success.message_type, StunMessageType::Response);
    assert!(success
        .attributes
        .iter()
        .all(|attr| attr.raw_type != 0x802C));

    let error = exchange(vec![raw_attribute(0x0003, &[0x00, 0x00, 0x00, 0x06])]).await;
    assert_eq!(error.message_type, StunMessageType::ErrorIndication);
    assert!(error.attributes.iter().all(|attr| attr.raw_type != 0x802C));
}

#[tokio::test]
async fn test_server_state_counters() {
    let state = Arc::new(server::ServerState::new());

    // Simulate concurrent counter increments.
    let mut handles = vec![];
    for _ in 0..100 {
        let s = state.clone();
        handles.push(tokio::spawn(async move {
            s.inc_requests();
        }));
    }
    for h in handles {
        h.await.unwrap();
    }

    assert_eq!(
        state
            .request_count
            .load(std::sync::atomic::Ordering::Relaxed),
        100
    );
}

// -----------------------------------------------------------------------
// Authentication flows over real UDP loopback (US3)
// -----------------------------------------------------------------------

const SHORT_TERM_CONFIG: &str = r#"
[auth]
mode = "short-term"

[[auth.users]]
username = "alice"
password = "s3cret"
"#;

const LONG_TERM_CONFIG: &str = r#"
[auth]
mode = "long-term"
realm = "example.org"
nonce_lifetime_sec = 600

[[auth.users]]
username = "alice"
password = "change-me"
"#;

fn auth_service(config_toml: &str) -> Arc<AuthService> {
    let config: config::Config = toml::from_str(config_toml).expect("parse auth config");
    Arc::new(AuthService::from_config(&config.auth).expect("auth service"))
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_secs()
}

fn request_bytes(attributes: Vec<StunAttribute>, key: Option<&[u8]>) -> Vec<u8> {
    let mut message = StunMessage::new(StunMessageType::Request, [0x5A; 12]);
    for attribute in attributes {
        message.add_attribute(attribute);
    }
    match key {
        Some(key) => message.encode_with_integrity(key),
        None => message.encode(),
    }
}

/// Send raw request bytes to a one-shot server and return the decoded response
/// and its wire bytes, or `None` when the server stays silent.
async fn exchange_with_service(
    service: Arc<AuthService>,
    request: Vec<u8>,
) -> Option<(StunMessage, Vec<u8>)> {
    let server_socket = tokio::net::UdpSocket::bind("127.0.0.1:0")
        .await
        .expect("bind server socket");
    let server_addr = server_socket.local_addr().unwrap();
    let server_socket = Arc::new(server_socket);

    let sock = server_socket.clone();
    tokio::spawn(async move {
        let mut buf = vec![0u8; 65536];
        let (len, client_addr) = sock.recv_from(&mut buf).await.unwrap();
        if let Some(response) = server::build_response_with_auth(&buf[..len], client_addr, &service)
        {
            let _ = sock.send_to(&response, client_addr).await;
        }
    });

    let client_socket = tokio::net::UdpSocket::bind("127.0.0.1:0")
        .await
        .expect("bind client socket");
    client_socket.send_to(&request, server_addr).await.unwrap();

    let mut resp_buf = vec![0u8; 65536];
    let received = tokio::time::timeout(
        Duration::from_secs(2),
        client_socket.recv_from(&mut resp_buf),
    )
    .await
    .ok()?;
    let (resp_len, _from) = received.ok()?;
    let bytes = resp_buf[..resp_len].to_vec();
    let decoded = StunMessage::decode(&bytes).expect("decode response");
    Some((decoded, bytes))
}

fn error_status(response: &StunMessage) -> u16 {
    let attribute = response
        .attributes
        .iter()
        .find(|attr| attr.attribute_type == StunAttributeType::ErrorCode)
        .expect("ERROR-CODE present");
    ErrorCode::decode_value(&attribute.attribute_value)
        .expect("ERROR-CODE decodes")
        .status_code()
}

fn has_type(response: &StunMessage, attribute_type: StunAttributeType) -> bool {
    response
        .attributes
        .iter()
        .any(|attr| attr.attribute_type == attribute_type)
}

fn attribute_value(response: &StunMessage, attribute_type: StunAttributeType) -> Option<Vec<u8>> {
    response
        .attributes
        .iter()
        .find(|attr| attr.attribute_type == attribute_type)
        .map(|attr| attr.attribute_value.clone())
}

fn assert_failure_has_no_integrity(response: &StunMessage) {
    assert!(
        !has_type(response, StunAttributeType::MessageIntegrity),
        "failure responses must not carry MESSAGE-INTEGRITY"
    );
    assert!(
        !has_type(response, StunAttributeType::Username),
        "failure responses must not echo USERNAME"
    );
}

#[tokio::test]
async fn test_auth_short_term_success_is_signed() {
    let service = auth_service(SHORT_TERM_CONFIG);
    let key = short_term_key("s3cret");
    let request = request_bytes(
        vec![StunAttribute::new(
            StunAttributeType::Username,
            b"alice".to_vec(),
        )],
        Some(&key),
    );

    let (response, bytes) = exchange_with_service(service, request)
        .await
        .expect("response");

    assert_eq!(response.message_type, StunMessageType::Response);
    assert!(has_type(&response, StunAttributeType::XorMappedAddress));
    assert!(has_type(&response, StunAttributeType::Software));
    assert!(!has_type(&response, StunAttributeType::Username));
    assert!(has_type(&response, StunAttributeType::MessageIntegrity));
    assert_eq!(
        response.attributes.last().unwrap().attribute_type,
        StunAttributeType::MessageIntegrity
    );
    assert!(StunMessage::verify_message_integrity(&bytes, &key));
}

#[tokio::test]
async fn test_auth_short_term_missing_credentials_400() {
    let service = auth_service(SHORT_TERM_CONFIG);
    let request = request_bytes(vec![], None);

    let (response, _) = exchange_with_service(service, request)
        .await
        .expect("response");

    assert_eq!(response.message_type, StunMessageType::ErrorIndication);
    assert_eq!(error_status(&response), 400);
    assert_main_types(&response);
    assert_failure_has_no_integrity(&response);
}

#[tokio::test]
async fn test_auth_short_term_unknown_user_401() {
    let service = auth_service(SHORT_TERM_CONFIG);
    let request = request_bytes(
        vec![StunAttribute::new(
            StunAttributeType::Username,
            b"bob".to_vec(),
        )],
        Some(&short_term_key("anything")),
    );

    let (response, _) = exchange_with_service(service, request)
        .await
        .expect("response");

    assert_eq!(response.message_type, StunMessageType::ErrorIndication);
    assert_eq!(error_status(&response), 401);
    assert_main_types(&response);
    assert_failure_has_no_integrity(&response);
}

#[tokio::test]
async fn test_auth_short_term_bad_mac_401() {
    let service = auth_service(SHORT_TERM_CONFIG);
    let request = request_bytes(
        vec![StunAttribute::new(
            StunAttributeType::Username,
            b"alice".to_vec(),
        )],
        Some(&short_term_key("wrong-secret")),
    );

    let (response, _) = exchange_with_service(service, request)
        .await
        .expect("response");

    assert_eq!(response.message_type, StunMessageType::ErrorIndication);
    assert_eq!(error_status(&response), 401);
    assert_main_types(&response);
    assert_failure_has_no_integrity(&response);
}

#[tokio::test]
async fn test_auth_long_term_missing_integrity_401_with_challenge() {
    let service = auth_service(LONG_TERM_CONFIG);
    let request = request_bytes(vec![], None);

    let (response, _) = exchange_with_service(service, request)
        .await
        .expect("response");

    assert_eq!(response.message_type, StunMessageType::ErrorIndication);
    assert_eq!(error_status(&response), 401);
    assert_eq!(
        attribute_value(&response, StunAttributeType::Realm).as_deref(),
        Some(b"example.org".as_slice())
    );
    let nonce = attribute_value(&response, StunAttributeType::Nonce).expect("NONCE present");
    assert_eq!(nonce.len(), 48, "NONCE must be 48 hex characters");
    assert!(nonce
        .iter()
        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(b)));
    assert_main_types(&response);
    assert_failure_has_no_integrity(&response);
}

#[tokio::test]
async fn test_auth_long_term_integrity_without_nonce_400() {
    let service = auth_service(LONG_TERM_CONFIG);
    let key = long_term_key("alice", "example.org", "change-me").unwrap();
    let request = request_bytes(
        vec![
            StunAttribute::new(StunAttributeType::Username, b"alice".to_vec()),
            StunAttribute::new(StunAttributeType::Realm, b"example.org".to_vec()),
        ],
        Some(&key),
    );

    let (response, _) = exchange_with_service(service, request)
        .await
        .expect("response");

    assert_eq!(response.message_type, StunMessageType::ErrorIndication);
    assert_eq!(error_status(&response), 400);
    assert_main_types(&response);
    assert_failure_has_no_integrity(&response);
}

#[tokio::test]
async fn test_auth_long_term_stale_nonce_438_with_fresh_challenge() {
    let config: config::Config = toml::from_str(LONG_TERM_CONFIG).unwrap();
    let service = Arc::new(AuthService::from_config(&config.auth).unwrap());
    let stale_nonce = service.issue_nonce_at(unix_now() - 601);
    let key = long_term_key("alice", "example.org", "change-me").unwrap();

    let request = request_bytes(
        vec![
            StunAttribute::new(StunAttributeType::Username, b"alice".to_vec()),
            StunAttribute::new(StunAttributeType::Realm, b"example.org".to_vec()),
            StunAttribute::new(StunAttributeType::Nonce, stale_nonce.into_bytes()),
        ],
        Some(&key),
    );

    let (response, _) = exchange_with_service(service, request)
        .await
        .expect("response");

    assert_eq!(response.message_type, StunMessageType::ErrorIndication);
    assert_eq!(error_status(&response), 438);
    assert_eq!(
        attribute_value(&response, StunAttributeType::Realm).as_deref(),
        Some(b"example.org".as_slice())
    );
    let fresh = attribute_value(&response, StunAttributeType::Nonce).expect("fresh NONCE");
    assert_eq!(fresh.len(), 48);
    assert_main_types(&response);
    assert_failure_has_no_integrity(&response);
}

#[tokio::test]
async fn test_auth_long_term_unknown_user_401_with_challenge() {
    let service = auth_service(LONG_TERM_CONFIG);
    let nonce = service.issue_nonce();
    let key = long_term_key("bob", "example.org", "whatever").unwrap();

    let request = request_bytes(
        vec![
            StunAttribute::new(StunAttributeType::Username, b"bob".to_vec()),
            StunAttribute::new(StunAttributeType::Realm, b"example.org".to_vec()),
            StunAttribute::new(StunAttributeType::Nonce, nonce.into_bytes()),
        ],
        Some(&key),
    );

    let (response, _) = exchange_with_service(service, request)
        .await
        .expect("response");

    assert_eq!(error_status(&response), 401);
    assert!(has_type(&response, StunAttributeType::Realm));
    assert!(has_type(&response, StunAttributeType::Nonce));
    assert_main_types(&response);
    assert_failure_has_no_integrity(&response);
}

#[tokio::test]
async fn test_auth_long_term_realm_mismatch_401() {
    let service = auth_service(LONG_TERM_CONFIG);
    let nonce = service.issue_nonce();
    let key = long_term_key("alice", "other.example", "change-me").unwrap();

    let request = request_bytes(
        vec![
            StunAttribute::new(StunAttributeType::Username, b"alice".to_vec()),
            StunAttribute::new(StunAttributeType::Realm, b"other.example".to_vec()),
            StunAttribute::new(StunAttributeType::Nonce, nonce.into_bytes()),
        ],
        Some(&key),
    );

    let (response, _) = exchange_with_service(service, request)
        .await
        .expect("response");

    assert_eq!(error_status(&response), 401);
    assert!(has_type(&response, StunAttributeType::Realm));
    assert!(has_type(&response, StunAttributeType::Nonce));
    assert_failure_has_no_integrity(&response);
}

#[tokio::test]
async fn test_auth_long_term_bad_mac_401() {
    let service = auth_service(LONG_TERM_CONFIG);
    let nonce = service.issue_nonce();
    let key = long_term_key("alice", "example.org", "wrong-password").unwrap();

    let request = request_bytes(
        vec![
            StunAttribute::new(StunAttributeType::Username, b"alice".to_vec()),
            StunAttribute::new(StunAttributeType::Realm, b"example.org".to_vec()),
            StunAttribute::new(StunAttributeType::Nonce, nonce.into_bytes()),
        ],
        Some(&key),
    );

    let (response, _) = exchange_with_service(service, request)
        .await
        .expect("response");

    assert_eq!(error_status(&response), 401);
    assert_failure_has_no_integrity(&response);
}

#[tokio::test]
async fn test_auth_long_term_success_is_signed() {
    let service = auth_service(LONG_TERM_CONFIG);
    let nonce = service.issue_nonce();
    let key = long_term_key("alice", "example.org", "change-me").unwrap();

    let request = request_bytes(
        vec![
            StunAttribute::new(StunAttributeType::Username, b"alice".to_vec()),
            StunAttribute::new(StunAttributeType::Realm, b"example.org".to_vec()),
            StunAttribute::new(StunAttributeType::Nonce, nonce.into_bytes()),
        ],
        Some(&key),
    );

    let (response, bytes) = exchange_with_service(service, request)
        .await
        .expect("response");

    assert_eq!(response.message_type, StunMessageType::Response);
    assert!(has_type(&response, StunAttributeType::XorMappedAddress));
    assert!(has_type(&response, StunAttributeType::Software));
    assert!(!has_type(&response, StunAttributeType::Username));
    assert!(!has_type(&response, StunAttributeType::Realm));
    assert!(!has_type(&response, StunAttributeType::Nonce));
    assert_eq!(
        response.attributes.last().unwrap().attribute_type,
        StunAttributeType::MessageIntegrity
    );
    assert!(StunMessage::verify_message_integrity(&bytes, &key));
}

#[tokio::test]
async fn test_auth_authenticated_420_is_signed() {
    let service = auth_service(SHORT_TERM_CONFIG);
    let key = short_term_key("s3cret");
    let request = request_bytes(
        vec![
            StunAttribute::new(StunAttributeType::Username, b"alice".to_vec()),
            raw_attribute(0x0024, &[0x00, 0x00, 0x00, 0x01]),
        ],
        Some(&key),
    );

    let (response, bytes) = exchange_with_service(service, request)
        .await
        .expect("response");

    assert_eq!(response.message_type, StunMessageType::ErrorIndication);
    assert_eq!(error_status(&response), 420);
    let unknown = response
        .attributes
        .iter()
        .find(|attr| attr.attribute_type == StunAttributeType::UnknownAttributes)
        .expect("UNKNOWN-ATTRIBUTES present");
    let unknown = UnknownAttributes::decode_value(&unknown.attribute_value).unwrap();
    assert_eq!(unknown.types, vec![0x0024]);
    assert!(has_type(&response, StunAttributeType::Software));
    assert_eq!(
        response.attributes.last().unwrap().attribute_type,
        StunAttributeType::MessageIntegrity
    );
    assert!(StunMessage::verify_message_integrity(&bytes, &key));
}

#[tokio::test]
async fn test_auth_enabled_modes_never_succeed_unauthenticated() {
    for config_toml in [SHORT_TERM_CONFIG, LONG_TERM_CONFIG] {
        let service = auth_service(config_toml);
        let request = request_bytes(vec![], None);
        let (response, _) = exchange_with_service(service, request)
            .await
            .expect("enabled mode must answer the challenge");
        assert_eq!(
            response.message_type,
            StunMessageType::ErrorIndication,
            "unauthenticated request must not succeed in an enabled mode"
        );
        assert!(matches!(error_status(&response), 400 | 401));
    }
}

#[tokio::test]
async fn test_auth_long_term_non_ascii_password_saslprep_end_to_end() {
    let decomposed_password = "pa\u{0308}ssword";
    let config_toml = format!(
        r#"
[auth]
mode = "long-term"
realm = "example.org"
nonce_lifetime_sec = 600

[[auth.users]]
username = "carol"
password = "{decomposed_password}"
"#
    );
    let service = auth_service(&config_toml);
    // The client signs with the precomposed spelling; SASLprep must make both
    // spellings derive the same key.
    let key = long_term_key("carol", "example.org", "pässword").unwrap();
    let nonce = service.issue_nonce();

    let request = request_bytes(
        vec![
            StunAttribute::new(StunAttributeType::Username, b"carol".to_vec()),
            StunAttribute::new(StunAttributeType::Realm, b"example.org".to_vec()),
            StunAttribute::new(StunAttributeType::Nonce, nonce.into_bytes()),
        ],
        Some(&key),
    );

    let (response, bytes) = exchange_with_service(service, request)
        .await
        .expect("response");

    assert_eq!(response.message_type, StunMessageType::Response);
    assert!(StunMessage::verify_message_integrity(&bytes, &key));
}

fn assert_main_types(response: &StunMessage) {
    assert!(
        has_type(response, StunAttributeType::Software),
        "every response must carry SOFTWARE"
    );
    assert_eq!(
        response.attributes.last().unwrap().attribute_type,
        StunAttributeType::Software,
        "failure responses must not sign MESSAGE-INTEGRITY"
    );
}

/// Auth service with authentication disabled for intake-only tests.
fn disabled_service() -> Arc<AuthService> {
    Arc::new(AuthService::disabled())
}

#[tokio::test]
async fn test_valid_terminal_fingerprint_returns_success() {
    let request = StunMessage::new(StunMessageType::Request, [0x61; 12]).encode_with_fingerprint();

    let (response, _) = exchange_with_service(disabled_service(), request)
        .await
        .expect("a valid terminal FINGERPRINT must be answered");

    assert_eq!(response.message_type, StunMessageType::Response);
    assert!(has_type(&response, StunAttributeType::XorMappedAddress));
}

#[tokio::test]
async fn test_bad_fingerprint_crc_silently_discarded() {
    let mut request =
        StunMessage::new(StunMessageType::Request, [0x62; 12]).encode_with_fingerprint();
    let last = request.len() - 1;
    request[last] ^= 0xFF;

    assert!(
        exchange_with_service(disabled_service(), request)
            .await
            .is_none(),
        "a FINGERPRINT with a bad CRC must be discarded silently"
    );
}

#[tokio::test]
async fn test_non_final_fingerprint_silently_discarded() {
    let mut message = StunMessage::new(StunMessageType::Request, [0x63; 12]);
    message.add_attribute(StunAttribute::new(
        StunAttributeType::Fingerprint,
        vec![0u8; 4],
    ));
    message.add_attribute(StunAttribute::new(
        StunAttributeType::Software,
        b"tail".to_vec(),
    ));

    assert!(
        exchange_with_service(disabled_service(), message.encode())
            .await
            .is_none(),
        "FINGERPRINT must be the final attribute"
    );
}

#[tokio::test]
async fn test_duplicate_fingerprint_silently_discarded() {
    let mut message = StunMessage::new(StunMessageType::Request, [0x64; 12]);
    message.add_attribute(StunAttribute::new(
        StunAttributeType::Fingerprint,
        vec![0u8; 4],
    ));
    let request = message.encode_with_fingerprint();

    assert!(
        exchange_with_service(disabled_service(), request)
            .await
            .is_none(),
        "duplicate FINGERPRINT attributes must be discarded silently"
    );
}

#[tokio::test]
async fn test_duplicate_integrity_silently_discarded() {
    let mut message = StunMessage::new(StunMessageType::Request, [0x65; 12]);
    message.add_attribute(StunAttribute::new(
        StunAttributeType::MessageIntegrity,
        vec![0u8; 20],
    ));
    let request = message.encode_with_integrity(b"ignored-key");

    assert!(
        exchange_with_service(disabled_service(), request)
            .await
            .is_none(),
        "duplicate MESSAGE-INTEGRITY attributes must be discarded silently"
    );
}

#[tokio::test]
async fn test_short_integrity_silently_discarded() {
    let mut request = StunMessage::new(StunMessageType::Request, [0x65; 12]).encode();
    request[3] = 20;
    request.extend_from_slice(&0x0008u16.to_be_bytes());
    request.extend_from_slice(&16u16.to_be_bytes());
    request.extend_from_slice(&[0u8; 16]);

    assert!(
        exchange_with_service(disabled_service(), request)
            .await
            .is_none(),
        "MESSAGE-INTEGRITY shorter than 20 bytes must be discarded silently"
    );
}

#[tokio::test]
async fn test_attribute_after_integrity_ignored_for_classification() {
    let mut message = StunMessage::new(StunMessageType::Request, [0x66; 12]);
    message.add_attribute(StunAttribute::new(
        StunAttributeType::MessageIntegrity,
        vec![0u8; 20],
    ));
    message.add_attribute(raw_attribute(0x0024, &[0x00, 0x00, 0x00, 0x00]));

    let (response, _) = exchange_with_service(disabled_service(), message.encode())
        .await
        .expect("attributes after MESSAGE-INTEGRITY are ignored");

    assert_eq!(
        response.message_type,
        StunMessageType::Response,
        "an unknown required attribute after MESSAGE-INTEGRITY must not trigger 420"
    );
}

#[tokio::test]
async fn test_integrity_followed_by_valid_fingerprint_succeeds() {
    let request = StunMessage::new(StunMessageType::Request, [0x67; 12])
        .encode_with_integrity_and_fingerprint(b"ignored-key");

    let (response, _) = exchange_with_service(disabled_service(), request)
        .await
        .expect("FINGERPRINT after MESSAGE-INTEGRITY must be accepted");

    assert_eq!(response.message_type, StunMessageType::Response);
}

#[tokio::test]
async fn test_valid_fingerprint_with_unknown_required_attribute_gets_420() {
    let mut message = StunMessage::new(StunMessageType::Request, [0x68; 12]);
    message.add_attribute(raw_attribute(0x0024, &[0x00, 0x00, 0x00, 0x00]));
    let request = message.encode_with_fingerprint();

    let (response, _) = exchange_with_service(disabled_service(), request)
        .await
        .expect("a valid FINGERPRINT must not suppress classification");

    assert_eq!(response.message_type, StunMessageType::ErrorIndication);
    assert_eq!(error_status(&response), 420);
}

#[tokio::test]
async fn test_defective_headers_silently_discarded() {
    let base = StunMessage::new(StunMessageType::Request, [0x69; 12]).encode();

    let mut leading_bits = base.clone();
    leading_bits[0] |= 0xC0;

    let mut cookie = base.clone();
    cookie[4] ^= 0xFF;

    let mut unaligned = base.clone();
    unaligned[3] = 2;
    unaligned.extend_from_slice(&[0x00, 0x00]);

    let mut beyond_datagram = base.clone();
    beyond_datagram[3] = 8;

    let mut truncated = base.clone();
    truncated[3] = 24;
    truncated.extend_from_slice(&0x0008u16.to_be_bytes());
    truncated.extend_from_slice(&20u16.to_be_bytes());

    for bytes in [leading_bits, cookie, unaligned, beyond_datagram, truncated] {
        assert!(
            exchange_with_service(disabled_service(), bytes)
                .await
                .is_none(),
            "a defective header or truncated attribute must be discarded silently"
        );
    }
}

#[tokio::test]
async fn test_non_request_classes_silently_discarded() {
    for message_type in [StunMessageType::Response, StunMessageType::ErrorIndication] {
        let bytes = StunMessage::new(message_type, [0x6A; 12]).encode();
        assert!(
            exchange_with_service(disabled_service(), bytes)
                .await
                .is_none(),
            "non-request classes must be discarded silently"
        );
    }

    let mut unsupported_method = StunMessage::new(StunMessageType::Request, [0x6B; 12]).encode();
    unsupported_method[1] = 0x05;
    assert!(
        exchange_with_service(disabled_service(), unsupported_method)
            .await
            .is_none(),
        "unsupported methods must be discarded silently"
    );
}

#[tokio::test]
async fn test_authentication_failure_wins_over_unknown_attribute() {
    let service = auth_service(LONG_TERM_CONFIG);
    let mut message = StunMessage::new(StunMessageType::Request, [0x6C; 12]);
    message.add_attribute(raw_attribute(0x0024, &[0x00, 0x00, 0x00, 0x00]));

    let (response, _) = exchange_with_service(service, message.encode())
        .await
        .expect("enabled authentication must answer with a challenge");

    assert_eq!(response.message_type, StunMessageType::ErrorIndication);
    assert_eq!(
        error_status(&response),
        401,
        "authentication failures must take precedence over 420"
    );
    assert!(
        !has_type(&response, StunAttributeType::UnknownAttributes),
        "an authentication failure must not list unknown attributes"
    );
}

#[tokio::test]
async fn test_every_response_carries_stable_software() {
    let mut responses = Vec::new();

    let request = StunMessage::new(StunMessageType::Request, [0x80; 12]).encode();
    responses.push(
        exchange_with_service(disabled_service(), request)
            .await
            .expect("success response")
            .0,
    );

    let mut message = StunMessage::new(StunMessageType::Request, [0x81; 12]);
    message.add_attribute(raw_attribute(0x0024, &[0x00, 0x00, 0x00, 0x00]));
    responses.push(
        exchange_with_service(disabled_service(), message.encode())
            .await
            .expect("420 response")
            .0,
    );

    let request = StunMessage::new(StunMessageType::Request, [0x82; 12]).encode();
    responses.push(
        exchange_with_service(auth_service(SHORT_TERM_CONFIG), request)
            .await
            .expect("400 response")
            .0,
    );

    let mut message = StunMessage::new(StunMessageType::Request, [0x83; 12]);
    message.add_attribute(StunAttribute::new(
        StunAttributeType::Username,
        b"mallory".to_vec(),
    ));
    message.add_attribute(StunAttribute::new(
        StunAttributeType::MessageIntegrity,
        vec![0u8; 20],
    ));
    responses.push(
        exchange_with_service(auth_service(SHORT_TERM_CONFIG), message.encode())
            .await
            .expect("401 response")
            .0,
    );

    let service = auth_service(LONG_TERM_CONFIG);
    let stale_nonce = service.issue_nonce_at(unix_now() - 3600);
    let key = long_term_key("alice", "example.org", "change-me").unwrap();
    let request = request_bytes(
        vec![
            StunAttribute::new(StunAttributeType::Username, b"alice".to_vec()),
            StunAttribute::new(StunAttributeType::Realm, b"example.org".to_vec()),
            StunAttribute::new(StunAttributeType::Nonce, stale_nonce.into_bytes()),
        ],
        Some(&key),
    );
    responses.push(
        exchange_with_service(service, request)
            .await
            .expect("438 response")
            .0,
    );

    assert_eq!(error_status(&responses[1]), 420);
    assert_eq!(error_status(&responses[2]), 400);
    assert_eq!(error_status(&responses[3]), 401);
    assert_eq!(error_status(&responses[4]), 438);

    let mut software_values = Vec::new();
    for response in &responses {
        assert!(
            has_type(response, StunAttributeType::Software),
            "every response must carry SOFTWARE"
        );
        let value = attribute_value(response, StunAttributeType::Software).expect("SOFTWARE value");
        let text = std::str::from_utf8(&value).expect("SOFTWARE must be valid UTF-8");
        assert!(
            text.chars().count() < 128,
            "SOFTWARE must be shorter than 128 characters"
        );
        assert_eq!(
            value,
            server::SOFTWARE.as_bytes(),
            "SOFTWARE must identify the server and version"
        );
        assert!(
            !has_type(response, StunAttributeType::OtherAddress),
            "responses must not carry OTHER-ADDRESS"
        );
        assert!(
            !has_type(response, StunAttributeType::ResponseOrigin),
            "responses must not carry RESPONSE-ORIGIN"
        );
        software_values.push(value);
    }

    assert!(software_values.windows(2).all(|pair| pair[0] == pair[1]));
}

#[tokio::test]
async fn test_response_arrives_from_bound_local_address_and_port() {
    let server_socket = tokio::net::UdpSocket::bind("127.0.0.1:0")
        .await
        .expect("bind server socket");
    let server_addr = server_socket.local_addr().unwrap();
    let server_socket = Arc::new(server_socket);

    let sock = server_socket.clone();
    tokio::spawn(async move {
        let mut buf = vec![0u8; 65536];
        let (len, client_addr) = sock.recv_from(&mut buf).await.unwrap();
        let response =
            server::build_response_with_auth(&buf[..len], client_addr, &AuthService::disabled())
                .expect("valid request");
        let _ = sock.send_to(&response, client_addr).await;
    });

    let client_socket = tokio::net::UdpSocket::bind("127.0.0.1:0")
        .await
        .expect("bind client socket");
    let request = StunMessage::new(StunMessageType::Request, [0x84; 12]).encode();
    client_socket.send_to(&request, server_addr).await.unwrap();

    let mut resp_buf = vec![0u8; 65536];
    let (_, from) = tokio::time::timeout(
        Duration::from_secs(2),
        client_socket.recv_from(&mut resp_buf),
    )
    .await
    .expect("response timeout")
    .expect("response");

    assert_eq!(
        from, server_addr,
        "responses must come from the bound local address and port"
    );
}
