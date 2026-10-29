//! Integration tests for STUN Server.
//!
//! Tests cover configuration loading from files, STUN request/response
//! over a real UDP loopback, and error handling for invalid configurations.

use std::io::Write;
use std::net::SocketAddr;
use std::sync::Arc;
use stun_protocol::{MappedAddress, StunAttributeType, StunMessage, StunMessageType};
use stunserver::config::{self, LogOutput};
use stunserver::server;
use tempfile::NamedTempFile;

/// Helper: write YAML to a temp file and return its path.
fn write_temp_config(yaml: &str) -> NamedTempFile {
    let mut f = NamedTempFile::new().expect("create temp file");
    f.write_all(yaml.as_bytes()).expect("write temp config");
    f
}

// -----------------------------------------------------------------------
// Configuration loading tests
// -----------------------------------------------------------------------

#[test]
fn test_load_config_from_file() {
    let yaml = r#"
server:
  listen: "127.0.0.1:13478"
logging:
  level: debug
  output: console
"#;
    let f = write_temp_config(yaml);
    let cfg = config::load_config(f.path()).unwrap();
    assert_eq!(
        cfg.server.listen,
        "127.0.0.1:13478".parse::<SocketAddr>().unwrap()
    );
    assert_eq!(cfg.logging.level.to_string(), "debug");
    assert_eq!(cfg.logging.output, LogOutput::Console);
}

#[test]
fn test_load_config_file_not_found() {
    let result = config::load_config(std::path::Path::new("/nonexistent/stunserver.yaml"));
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("not found"));
}

#[test]
fn test_load_config_invalid_yaml() {
    let yaml = "{{invalid yaml::";
    let f = write_temp_config(yaml);
    let result = config::load_config(f.path());
    assert!(result.is_err());
}

#[test]
fn test_load_config_invalid_log_level() {
    let yaml = r#"
logging:
  level: verbose
  output: console
"#;
    let f = write_temp_config(yaml);
    let result = config::load_config(f.path());
    assert!(result.is_err());
}

#[test]
fn test_load_config_missing_file_section() {
    let yaml = r#"
logging:
  output: file
"#;
    let f = write_temp_config(yaml);
    let result = config::load_config(f.path());
    // Parses OK but validation should fail.
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err
        .to_string()
        .contains("File logging configuration required"));
}

#[test]
fn test_load_config_with_file_logging() {
    let yaml = r#"
server:
  listen: "0.0.0.0:3478"
logging:
  level: info
  output: file
  file:
    path: "/tmp/stunserver-test.log"
    max_size: "50MB"
    rotation: daily
    max_files: 5
"#;
    let f = write_temp_config(yaml);
    let cfg = config::load_config(f.path()).unwrap();
    let file_cfg = cfg.logging.file.unwrap();
    assert_eq!(file_cfg.path.to_str().unwrap(), "/tmp/stunserver-test.log");
    assert_eq!(file_cfg.max_size.0, 50 * 1024 * 1024);
    assert_eq!(file_cfg.max_files, 5);
}

#[test]
fn test_default_config_values() {
    let cfg = config::default_config();
    assert_eq!(
        cfg.server.listen,
        "0.0.0.0:3478".parse::<SocketAddr>().unwrap()
    );
    assert_eq!(cfg.logging.level.to_string(), "info");
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
