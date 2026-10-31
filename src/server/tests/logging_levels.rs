//! End-to-end tests for log level behavior.
//!
//! Starts the real `stunserver` binary with `--trace` or `--debug`, sends a
//! single Binding request over UDP loopback, and inspects the captured stderr.

use std::io::{BufRead, BufReader};
use std::net::UdpSocket;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use stun_protocol::integrity::long_term_key;
use stun_protocol::{StunAttribute, StunAttributeType, StunMessage, StunMessageType};

const TRANSACTION_ID: [u8; 12] = [0x5A; 12];

struct CaseResult {
    ready: bool,
    response_ok: bool,
    stderr: String,
    transaction_hex: String,
    dir: tempfile::TempDir,
}

/// Grab an ephemeral UDP port on loopback by binding and releasing it.
fn free_udp_port() -> u16 {
    let socket = UdpSocket::bind("127.0.0.1:0").expect("bind ephemeral port");
    socket.local_addr().unwrap().port()
}

/// Write a config that keeps log files inside the temp directory, enables
/// console output, and appends extra TOML sections (for `[auth]`).
fn write_config_with(dir: &Path, extra: &str) -> PathBuf {
    let config_path = dir.join("stunserver.toml");
    let log_dir = dir.join("log");
    let toml = format!(
        r#"
[logging]
level = "info"
log_dir = "{}"
file_prefix = "stunserver"
enable_console = true
{extra}
"#,
        log_dir.display()
    );
    std::fs::write(&config_path, toml).expect("write temp config");
    config_path
}

/// Spawn the server binary with the given level flag.
fn spawn_server(port: u16, config: &Path, flag: &str) -> Child {
    Command::new(env!("CARGO_BIN_EXE_stunserver"))
        .arg("--config")
        .arg(config)
        .arg("--listen")
        .arg(format!("127.0.0.1:{}", port))
        .arg(flag)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn stunserver binary")
}

/// Send exactly one Binding request and wait for its response.
fn send_single_request(port: u16) -> bool {
    let mut request = StunMessage::new(StunMessageType::Request, TRANSACTION_ID);
    request.add_attribute(StunAttribute::new(
        StunAttributeType::Software,
        b"test-client".to_vec(),
    ));
    send_request_bytes(port, &request.encode(), true)
}

/// Send arbitrary request bytes and report whether a success Response arrived.
fn send_request_bytes(port: u16, request: &[u8], expect_success: bool) -> bool {
    let client = UdpSocket::bind("127.0.0.1:0").expect("bind client");
    client
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("set timeout");

    client
        .send_to(request, ("127.0.0.1", port))
        .expect("send request");

    let mut buf = [0u8; 1500];
    match client.recv_from(&mut buf) {
        Ok((len, _)) => match StunMessage::decode(&buf[..len]) {
            Ok(response) => {
                let matched = response.transaction_id == TRANSACTION_ID;
                if expect_success {
                    response.message_type == StunMessageType::Response && matched
                } else {
                    matched
                }
            }
            Err(_) => false,
        },
        Err(_) => false,
    }
}

/// Run one server instance, wait for it to listen, send a single request, then
/// capture stderr.
fn run_case(flag: &str) -> CaseResult {
    run_case_with(flag, "")
}

/// Run one server instance with extra config TOML, wait for it to listen, send
/// a plain Binding request, then capture stderr.
fn run_case_with(flag: &str, extra: &str) -> CaseResult {
    run_case_with_request(flag, extra, None)
}

/// Run one server instance, optionally sending a custom request, then capture
/// stderr.
fn run_case_with_request(flag: &str, extra: &str, request: Option<Vec<u8>>) -> CaseResult {
    let dir = tempfile::tempdir().expect("create temp dir");
    let config = write_config_with(dir.path(), extra);
    let port = free_udp_port();
    let mut child = spawn_server(port, &config, flag);

    let stderr = child.stderr.take().expect("piped stderr");
    let captured = Arc::new(Mutex::new(String::new()));
    let captured_reader = captured.clone();
    let reader = thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            let mut guard = captured_reader.lock().expect("lock captured stderr");
            guard.push_str(&line);
            guard.push('\n');
        }
    });

    // Wait until the server reports that it is listening.
    let marker = format!("STUN server listening on 127.0.0.1:{}", port);
    let deadline = Instant::now() + Duration::from_secs(5);
    let ready = loop {
        if captured
            .lock()
            .expect("lock captured stderr")
            .contains(&marker)
        {
            break true;
        }
        if Instant::now() >= deadline {
            break false;
        }
        thread::sleep(Duration::from_millis(50));
    };

    let response_ok = ready
        && match &request {
            Some(bytes) => send_request_bytes(port, bytes, false),
            None => send_single_request(port),
        };

    // Give the console logger a moment to emit the request records.
    thread::sleep(Duration::from_millis(300));

    let _ = child.kill();
    let _ = child.wait();
    let _ = reader.join();

    let stderr = captured.lock().expect("lock captured stderr").clone();
    let transaction_hex = TRANSACTION_ID
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect();

    CaseResult {
        ready,
        response_ok,
        stderr,
        transaction_hex,
        dir,
    }
}

#[test]
fn test_trace_flag_emits_raw_packets_and_debug_records() {
    let result = run_case("--trace");
    assert!(
        result.ready,
        "--trace server never started; stderr:\n{}",
        result.stderr
    );
    assert!(
        result.response_ok,
        "--trace server did not answer the Binding request; stderr:\n{}",
        result.stderr
    );
    assert!(
        result.stderr.contains("[TRACE] Received: ["),
        "missing TRACE received bytes; stderr:\n{}",
        result.stderr
    );
    assert!(
        result.stderr.contains("[TRACE] Response: ["),
        "missing TRACE response bytes; stderr:\n{}",
        result.stderr
    );
    assert!(
        result.stderr.contains("[DEBUG]"),
        "TRACE should include DEBUG records; stderr:\n{}",
        result.stderr
    );
}

#[test]
fn test_debug_flag_emits_summaries_without_raw_packets() {
    let result = run_case("--debug");
    assert!(
        result.ready,
        "--debug server never started; stderr:\n{}",
        result.stderr
    );
    assert!(
        result.response_ok,
        "--debug server did not answer the Binding request; stderr:\n{}",
        result.stderr
    );

    assert_eq!(
        result.stderr.matches("Request from 127.0.0.1:").count(),
        1,
        "expected exactly one Request from record; stderr:\n{}",
        result.stderr
    );
    assert_eq!(
        result
            .stderr
            .matches("Received: Binding Request txn=")
            .count(),
        1,
        "expected exactly one received summary; stderr:\n{}",
        result.stderr
    );
    assert_eq!(
        result
            .stderr
            .matches("Response: Binding Success Response txn=")
            .count(),
        1,
        "expected exactly one response summary; stderr:\n{}",
        result.stderr
    );
    assert!(
        result
            .stderr
            .contains(&format!("txn={}", result.transaction_hex)),
        "summary must contain the transaction ID; stderr:\n{}",
        result.stderr
    );
    assert!(
        result.stderr.contains("SOFTWARE=\"test-client\""),
        "received summary must describe SOFTWARE; stderr:\n{}",
        result.stderr
    );
    assert!(
        result.stderr.contains("XOR-MAPPED-ADDRESS="),
        "response summary must describe the mapped address; stderr:\n{}",
        result.stderr
    );

    assert!(
        !result.stderr.contains("[TRACE]"),
        "DEBUG must not include TRACE records; stderr:\n{}",
        result.stderr
    );
    assert!(
        !result.stderr.contains("Received: [") && !result.stderr.contains("Response: ["),
        "DEBUG must not include raw packet bytes; stderr:\n{}",
        result.stderr
    );

    drop(result.dir);
}

#[test]
fn test_debug_summary_renders_known_names_and_unknown_codes() {
    fn raw_attribute(raw_type: u16, value: &[u8]) -> StunAttribute {
        let mut bytes = Vec::with_capacity(4 + value.len());
        bytes.extend_from_slice(&raw_type.to_be_bytes());
        bytes.extend_from_slice(&(value.len() as u16).to_be_bytes());
        bytes.extend_from_slice(value);
        StunAttribute::decode(&bytes).expect("raw attribute")
    }

    let mut request = StunMessage::new(StunMessageType::Request, TRANSACTION_ID);
    request.add_attribute(StunAttribute::new(
        StunAttributeType::Software,
        b"test-client".to_vec(),
    ));
    request.add_attribute(raw_attribute(0x0024, &[0x00, 0x00, 0x00, 0x01]));
    let result = run_case_with_request("--debug", "", Some(request.encode()));

    assert!(
        result.ready,
        "server never started; stderr:\n{}",
        result.stderr
    );
    assert!(
        result.stderr.contains("SOFTWARE=\"test-client\""),
        "known attributes must render by RFC name; stderr:\n{}",
        result.stderr
    );
    assert!(
        result.stderr.contains("0x0024(len=4)"),
        "unknown attributes must render as 0xNNNN; stderr:\n{}",
        result.stderr
    );
    assert!(
        result.stderr.contains("ERROR-CODE=420"),
        "the 420 response must be summarized; stderr:\n{}",
        result.stderr
    );
    assert!(
        !result.stderr.contains("Received: [") && !result.stderr.contains("Response: ["),
        "DEBUG must not include raw packet bytes; stderr:\n{}",
        result.stderr
    );
    drop(result.dir);
}

// -----------------------------------------------------------------------
// Secret redaction (FR-033, SC-010)
// -----------------------------------------------------------------------

fn hex_lower(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

fn signed_request(attributes: Vec<StunAttribute>, key: &[u8]) -> Vec<u8> {
    let mut request = StunMessage::new(StunMessageType::Request, TRANSACTION_ID);
    for attribute in attributes {
        request.add_attribute(attribute);
    }
    request.encode_with_integrity(key)
}

#[test]
fn test_auth_short_term_password_never_logged() {
    let auth = r#"
[auth]
mode = "short-term"

[[auth.users]]
username = "alice"
password = "short-term-password-xyz"
"#;
    let key = b"short-term-password-xyz";
    let request = signed_request(
        vec![StunAttribute::new(
            StunAttributeType::Username,
            b"alice".to_vec(),
        )],
        key,
    );

    let result = run_case_with_request("--debug", auth, Some(request));
    assert!(
        result.ready,
        "server never started; stderr:\n{}",
        result.stderr
    );
    assert!(
        !result.stderr.contains("short-term-password-xyz"),
        "short-term password must never be logged; stderr:\n{}",
        result.stderr
    );
    assert!(
        !result.stderr.contains(&hex_lower(key)),
        "short-term key must never be logged as hex; stderr:\n{}",
        result.stderr
    );
    assert!(
        result.stderr.contains("MESSAGE-INTEGRITY(len=20)"),
        "MESSAGE-INTEGRITY must be summarized by length only; stderr:\n{}",
        result.stderr
    );
    assert!(
        result.stderr.contains("USERNAME=\"alice\""),
        "USERNAME is not a secret and may be logged; stderr:\n{}",
        result.stderr
    );
    drop(result.dir);
}

#[test]
fn test_auth_long_term_password_and_key_never_logged() {
    let auth = r#"
[auth]
mode = "long-term"
realm = "example.org"
nonce_lifetime_sec = 600

[[auth.users]]
username = "alice"
password = "hunter2-secret"
"#;
    let key = long_term_key("alice", "example.org", "hunter2-secret").unwrap();
    let request = signed_request(
        vec![
            StunAttribute::new(StunAttributeType::Username, b"alice".to_vec()),
            StunAttribute::new(StunAttributeType::Realm, b"example.org".to_vec()),
        ],
        &key,
    );

    let result = run_case_with_request("--trace", auth, Some(request));
    assert!(
        result.ready,
        "server never started; stderr:\n{}",
        result.stderr
    );
    assert!(
        !result.stderr.contains("hunter2-secret"),
        "long-term password must never be logged; stderr:\n{}",
        result.stderr
    );
    assert!(
        !result.stderr.contains("hunter2"),
        "password fragment must never be logged; stderr:\n{}",
        result.stderr
    );
    assert!(
        !result.stderr.contains(&hex_lower(&key)),
        "derived long-term key must never be logged; stderr:\n{}",
        result.stderr
    );
    assert!(
        result.stderr.contains("MESSAGE-INTEGRITY(len=20)"),
        "MESSAGE-INTEGRITY must be summarized by length only; stderr:\n{}",
        result.stderr
    );
    assert!(
        !result.stderr.contains("NONCE=\"") && !result.stderr.contains("NONCE="),
        "the server nonce must not be rendered as a decodable value in summaries; stderr:\n{}",
        result.stderr
    );
    drop(result.dir);
}
