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

/// Write a config that keeps log files inside the temp directory and enables
/// console output, then return its path.
fn write_config(dir: &Path) -> PathBuf {
    let config_path = dir.join("stunserver.toml");
    let log_dir = dir.join("log");
    let toml = format!(
        r#"
[logging]
level = "info"
log_dir = "{}"
file_prefix = "stunserver"
enable_console = true
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
    let client = UdpSocket::bind("127.0.0.1:0").expect("bind client");
    client
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("set timeout");

    let mut request = StunMessage::new(StunMessageType::Request, TRANSACTION_ID);
    request.add_attribute(StunAttribute::new(
        StunAttributeType::Software,
        b"test-client".to_vec(),
    ));

    client
        .send_to(&request.encode(), ("127.0.0.1", port))
        .expect("send request");

    let mut buf = [0u8; 1500];
    match client.recv_from(&mut buf) {
        Ok((len, _)) => match StunMessage::decode(&buf[..len]) {
            Ok(response) => {
                response.message_type == StunMessageType::Response
                    && response.transaction_id == TRANSACTION_ID
            }
            Err(_) => false,
        },
        Err(_) => false,
    }
}

/// Run one server instance, wait for it to listen, send a single request, then
/// capture stderr.
fn run_case(flag: &str) -> CaseResult {
    let dir = tempfile::tempdir().expect("create temp dir");
    let config = write_config(dir.path());
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

    let response_ok = ready && send_single_request(port);

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
