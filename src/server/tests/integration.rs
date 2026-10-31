//! Integration tests for STUN Server.
//!
//! Tests cover configuration loading from files, STUN request/response
//! over a real UDP loopback, and error handling for invalid configurations.

use std::io::Write;
use std::sync::Arc;
use stun_protocol::{MappedAddress, StunAttributeType, StunMessage, StunMessageType};
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
    assert_eq!(response.attributes.len(), 1);
    assert_eq!(
        response.attributes[0].attribute_type,
        StunAttributeType::XorMappedAddress
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
    assert_eq!(response.attributes.len(), 2);

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
    assert_eq!(response.attributes.len(), 1);
    assert_eq!(
        response.attributes[0].attribute_type,
        StunAttributeType::XorMappedAddress
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
