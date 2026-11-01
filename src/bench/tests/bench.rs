//! Integration tests for STUN Benchmark tool.
//!
//! Tests cover statistics calculation, latency percentiles, authentication,
//! transaction-ID matching, and end-to-end benchmark execution against a mock
//! STUN server.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use stun_protocol::{
    integrity, ErrorCode, MappedAddress, StunAttribute, StunAttributeType, StunMessage,
    StunMessageType,
};
use stunbench::bench::{self, AuthMode, BenchAuth, BenchConfig};
use stunbench::stats::{BenchmarkStats, LatencyCollector};
use tokio::net::UdpSocket;

fn disabled_auth() -> bench::PreparedAuth {
    BenchAuth::disabled().prepare().unwrap()
}

fn has_text_attribute(
    message: &StunMessage,
    attribute_type: StunAttributeType,
    value: &str,
) -> bool {
    message.attributes.iter().any(|a| {
        a.attribute_type == attribute_type && a.attribute_value.as_slice() == value.as_bytes()
    })
}

fn error_response(transaction_id: [u8; 12], class: u8, number: u8, reason: &str) -> StunMessage {
    let mut response = StunMessage::new(StunMessageType::ErrorResponse, transaction_id);
    let code = ErrorCode::new(class, number, reason.to_string()).expect("valid error code");
    response.add_attribute(StunAttribute::new(
        StunAttributeType::ErrorCode,
        code.encode_value(),
    ));
    response
}

fn success_response(transaction_id: [u8; 12], from: SocketAddr) -> StunMessage {
    let mut response = StunMessage::new(StunMessageType::Response, transaction_id);
    let mapped = MappedAddress::new(from.ip(), from.port());
    let xor_value = mapped.encode_xor(response.magic_cookie, &response.transaction_id);
    response.add_attribute(StunAttribute::new(
        StunAttributeType::XorMappedAddress,
        xor_value,
    ));
    response
}

/// Spawn a mock STUN server that responds to all binding requests.
async fn spawn_mock_server() -> SocketAddr {
    let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let addr = socket.local_addr().unwrap();
    let socket = Arc::new(socket);

    tokio::spawn(async move {
        let mut buf = vec![0u8; 65536];
        for _ in 0..10000 {
            let (len, from) = match socket.recv_from(&mut buf).await {
                Ok(r) => r,
                Err(_) => break,
            };

            if let Ok(request) = StunMessage::decode(&buf[..len]) {
                if request.message_type == StunMessageType::Request {
                    let mut response =
                        StunMessage::new(StunMessageType::Response, request.transaction_id);
                    let mapped = MappedAddress::new(from.ip(), from.port());
                    let xor_value =
                        mapped.encode_xor(response.magic_cookie, &response.transaction_id);
                    let attr = StunAttribute::new(StunAttributeType::XorMappedAddress, xor_value);
                    response.add_attribute(attr);
                    let _ = socket.send_to(&response.encode(), from).await;
                }
            }
        }
    });

    addr
}

/// Spawn a mock server that sleeps before answering every request.
async fn spawn_delayed_server(delay: Duration) -> SocketAddr {
    let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let addr = socket.local_addr().unwrap();
    let socket = Arc::new(socket);

    tokio::spawn(async move {
        let mut buf = vec![0u8; 65536];
        loop {
            let Ok((len, from)) = socket.recv_from(&mut buf).await else {
                break;
            };
            let Ok(request) = StunMessage::decode(&buf[..len]) else {
                continue;
            };
            if request.message_type != StunMessageType::Request {
                continue;
            }
            tokio::time::sleep(delay).await;
            let response = success_response(request.transaction_id, from);
            let _ = socket.send_to(&response.encode(), from).await;
        }
    });

    addr
}

/// Spawn a mock server that emits a response with a wrong transaction ID
/// before the real one.
async fn spawn_spoofing_server() -> SocketAddr {
    let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let addr = socket.local_addr().unwrap();
    let socket = Arc::new(socket);

    tokio::spawn(async move {
        let mut buf = vec![0u8; 65536];
        loop {
            let Ok((len, from)) = socket.recv_from(&mut buf).await else {
                break;
            };
            let Ok(request) = StunMessage::decode(&buf[..len]) else {
                continue;
            };
            if request.message_type != StunMessageType::Request {
                continue;
            }
            let spoofed = success_response([0xEE; 12], from);
            let _ = socket.send_to(&spoofed.encode(), from).await;
            let response = success_response(request.transaction_id, from);
            let _ = socket.send_to(&response.encode(), from).await;
        }
    });

    addr
}

/// Spawn a server that binds but never answers.
async fn spawn_dropping_server() -> SocketAddr {
    let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let addr = socket.local_addr().unwrap();
    tokio::spawn(async move {
        let _socket = socket;
        std::future::pending::<()>().await;
    });
    addr
}

/// Spawn a mock short-term protected server.
///
/// Requests must carry USERNAME and a valid MESSAGE-INTEGRITY computed from
/// `request_password`. Responses are signed with `response_password`, which
/// lets tests exercise invalid response integrity by using a different value.
async fn spawn_short_term_server(
    username: &str,
    request_password: &str,
    response_password: &str,
) -> (SocketAddr, Arc<AtomicUsize>) {
    let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let addr = socket.local_addr().unwrap();
    let socket = Arc::new(socket);
    let request_key = integrity::short_term_key(request_password).unwrap();
    let response_key = integrity::short_term_key(response_password).unwrap();
    let username = username.to_string();
    let accepted = Arc::new(AtomicUsize::new(0));
    let counter = accepted.clone();

    tokio::spawn(async move {
        let mut buf = vec![0u8; 65536];
        loop {
            let Ok((len, from)) = socket.recv_from(&mut buf).await else {
                break;
            };
            let data = &buf[..len];
            let Ok(request) = StunMessage::decode(data) else {
                continue;
            };
            if request.message_type != StunMessageType::Request {
                continue;
            }

            let authentic = has_text_attribute(&request, StunAttributeType::Username, &username)
                && StunMessage::verify_message_integrity(data, &request_key);
            if authentic {
                let response = success_response(request.transaction_id, from);
                let _ = socket
                    .send_to(&response.encode_with_integrity(&response_key), from)
                    .await;
                counter.fetch_add(1, Ordering::Relaxed);
            } else {
                // RFC 5389 section 10.1.2: challenges are not signed.
                let response = error_response(request.transaction_id, 4, 1, "Unauthorized");
                let _ = socket.send_to(&response.encode(), from).await;
            }
        }
    });

    (addr, accepted)
}

/// Spawn a mock long-term protected server that challenges unsigned requests.
async fn spawn_long_term_server(
    username: &str,
    password: &str,
    realm: &str,
    nonce: &str,
) -> (SocketAddr, Arc<AtomicUsize>) {
    let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let addr = socket.local_addr().unwrap();
    let socket = Arc::new(socket);
    let key = integrity::long_term_key(username, realm, password).unwrap();
    let username = username.to_string();
    let realm = realm.to_string();
    let nonce = nonce.to_string();
    let accepted = Arc::new(AtomicUsize::new(0));
    let counter = accepted.clone();

    tokio::spawn(async move {
        let mut buf = vec![0u8; 65536];
        loop {
            let Ok((len, from)) = socket.recv_from(&mut buf).await else {
                break;
            };
            let data = &buf[..len];
            let Ok(request) = StunMessage::decode(data) else {
                continue;
            };
            if request.message_type != StunMessageType::Request {
                continue;
            }

            let has_integrity =
                request.attribute_count(StunAttributeType::MessageIntegrity as u16) > 0;
            if !has_integrity {
                let mut response = error_response(request.transaction_id, 4, 1, "Unauthorized");
                response.add_attribute(StunAttribute::new(
                    StunAttributeType::Realm,
                    realm.clone().into_bytes(),
                ));
                response.add_attribute(StunAttribute::new(
                    StunAttributeType::Nonce,
                    nonce.clone().into_bytes(),
                ));
                let _ = socket.send_to(&response.encode(), from).await;
                continue;
            }

            let authentic = has_text_attribute(&request, StunAttributeType::Username, &username)
                && has_text_attribute(&request, StunAttributeType::Realm, &realm)
                && has_text_attribute(&request, StunAttributeType::Nonce, &nonce)
                && StunMessage::verify_message_integrity(data, &key);
            if authentic {
                let response = success_response(request.transaction_id, from);
                let _ = socket
                    .send_to(&response.encode_with_integrity(&key), from)
                    .await;
                counter.fetch_add(1, Ordering::Relaxed);
            } else {
                let response = error_response(request.transaction_id, 4, 1, "Unauthorized");
                let _ = socket.send_to(&response.encode(), from).await;
            }
        }
    });

    (addr, accepted)
}

/// Spawn a long-term mock that rotates its nonce after `requests_per_nonce`
/// authenticated successes, then answers the next valid request with 438
/// Stale Nonce and the new nonce.
async fn spawn_rotating_nonce_server(
    username: &str,
    password: &str,
    realm: &str,
    nonce_prefix: &str,
    requests_per_nonce: usize,
) -> (SocketAddr, Arc<AtomicUsize>, Arc<AtomicUsize>) {
    let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let addr = socket.local_addr().unwrap();
    let socket = Arc::new(socket);
    let key = integrity::long_term_key(username, realm, password).unwrap();
    let username = username.to_string();
    let realm = realm.to_string();
    let nonce_prefix = nonce_prefix.to_string();
    let accepted = Arc::new(AtomicUsize::new(0));
    let stale = Arc::new(AtomicUsize::new(0));
    let accepted_counter = accepted.clone();
    let stale_counter = stale.clone();

    tokio::spawn(async move {
        let mut buf = vec![0u8; 65536];
        let mut generation = 1usize;
        let mut nonce = format!("{nonce_prefix}-{generation}");
        let mut count = 0usize;

        let challenge = |transaction_id, nonce: &str| {
            let mut response = error_response(transaction_id, 4, 38, "Stale Nonce");
            response.add_attribute(StunAttribute::new(
                StunAttributeType::Realm,
                realm.clone().into_bytes(),
            ));
            response.add_attribute(StunAttribute::new(
                StunAttributeType::Nonce,
                nonce.as_bytes().to_vec(),
            ));
            response
        };

        loop {
            let Ok((len, from)) = socket.recv_from(&mut buf).await else {
                break;
            };
            let data = &buf[..len];
            let Ok(request) = StunMessage::decode(data) else {
                continue;
            };
            if request.message_type != StunMessageType::Request {
                continue;
            }

            let has_integrity =
                request.attribute_count(StunAttributeType::MessageIntegrity as u16) > 0;
            if !has_integrity {
                let mut response = error_response(request.transaction_id, 4, 1, "Unauthorized");
                response.add_attribute(StunAttribute::new(
                    StunAttributeType::Realm,
                    realm.clone().into_bytes(),
                ));
                response.add_attribute(StunAttribute::new(
                    StunAttributeType::Nonce,
                    nonce.clone().into_bytes(),
                ));
                let _ = socket.send_to(&response.encode(), from).await;
                continue;
            }

            let credentials_ok =
                has_text_attribute(&request, StunAttributeType::Username, &username)
                    && has_text_attribute(&request, StunAttributeType::Realm, &realm)
                    && StunMessage::verify_message_integrity(data, &key);
            let nonce_matches = has_text_attribute(&request, StunAttributeType::Nonce, &nonce);

            if credentials_ok && nonce_matches && count >= requests_per_nonce {
                generation += 1;
                nonce = format!("{nonce_prefix}-{generation}");
                count = 0;
                stale_counter.fetch_add(1, Ordering::Relaxed);
                let response = challenge(request.transaction_id, &nonce);
                let _ = socket.send_to(&response.encode(), from).await;
            } else if credentials_ok && nonce_matches {
                count += 1;
                let response = success_response(request.transaction_id, from);
                let _ = socket
                    .send_to(&response.encode_with_integrity(&key), from)
                    .await;
                accepted_counter.fetch_add(1, Ordering::Relaxed);
            } else if credentials_ok {
                stale_counter.fetch_add(1, Ordering::Relaxed);
                let response = challenge(request.transaction_id, &nonce);
                let _ = socket.send_to(&response.encode(), from).await;
            } else {
                let response = error_response(request.transaction_id, 4, 1, "Unauthorized");
                let _ = socket.send_to(&response.encode(), from).await;
            }
        }
    });

    (addr, accepted, stale)
}

#[tokio::test]
async fn test_benchmark_short_duration() {
    let server_addr = spawn_mock_server().await;

    let config = BenchConfig {
        server_addrs: vec![server_addr],
        target_qps: 100,
        duration: Some(Duration::from_secs(2)),
        request_count: None,
        fingerprint: false,
        auth: BenchAuth::disabled(),
    };

    let stats = bench::run_benchmark(&config).await.unwrap();

    assert!(stats.total_requests > 0);
    assert!(stats.successful_requests > 0);
    assert_eq!(stats.target_qps, 100);
    assert!(stats.duration.as_secs() >= 2);
}

#[tokio::test]
async fn test_benchmark_returns_without_waiting_for_progress_interval() {
    let server_addr = spawn_mock_server().await;

    let config = BenchConfig {
        server_addrs: vec![server_addr],
        target_qps: 100,
        duration: Some(Duration::from_millis(300)),
        request_count: None,
        fingerprint: false,
        auth: BenchAuth::disabled(),
    };

    let start = Instant::now();
    let stats = bench::run_benchmark(&config).await.unwrap();
    let elapsed = start.elapsed();

    assert!(stats.successful_requests > 0);
    // The progress reporter must poll its stop flag every second, not once
    // per 10-second report interval.
    assert!(
        elapsed < Duration::from_secs(5),
        "benchmark took {:?}; the progress task delays shutdown",
        elapsed
    );
}

#[tokio::test]
async fn test_stunbench_prints_final_progress_line() {
    let server_addr = spawn_mock_server().await;

    let output = tokio::process::Command::new(env!("CARGO_BIN_EXE_stunbench"))
        .args([
            "--qps",
            "100",
            "--duration",
            "1",
            "--quiet",
            &server_addr.to_string(),
        ])
        .output()
        .await
        .expect("stunbench should run");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("[00:01] Sent:"),
        "missing final progress line; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("Benchmark Complete"),
        "missing final summary; stdout:\n{stdout}"
    );
}

#[tokio::test]
async fn test_benchmark_request_count() {
    let server_addr = spawn_mock_server().await;

    let config = BenchConfig {
        server_addrs: vec![server_addr],
        target_qps: 500,
        duration: None,
        request_count: Some(100),
        fingerprint: false,
        auth: BenchAuth::disabled(),
    };

    let stats = bench::run_benchmark(&config).await.unwrap();

    // Should have sent approximately 100 requests.
    assert!(stats.total_requests >= 90 && stats.total_requests <= 110);
}

#[tokio::test]
async fn test_benchmark_latency_measures_real_round_trip() {
    let server_addr = spawn_delayed_server(Duration::from_millis(30)).await;

    let config = BenchConfig {
        server_addrs: vec![server_addr],
        target_qps: 100,
        duration: None,
        request_count: Some(10),
        fingerprint: false,
        auth: BenchAuth::disabled(),
    };

    let stats = bench::run_benchmark(&config).await.unwrap();

    assert_eq!(stats.successful_requests, 10);
    // The old implementation reported a fixed 500µs placeholder.
    assert!(
        stats.latency_p50 >= Duration::from_millis(20),
        "p50 latency {:?} does not reflect the 30ms server delay",
        stats.latency_p50
    );
}

#[tokio::test]
async fn test_benchmark_ignores_responses_with_unknown_transaction_id() {
    let server_addr = spawn_spoofing_server().await;

    let config = BenchConfig {
        server_addrs: vec![server_addr],
        target_qps: 500,
        duration: None,
        request_count: Some(10),
        fingerprint: false,
        auth: BenchAuth::disabled(),
    };

    let stats = bench::run_benchmark(&config).await.unwrap();

    // Each request yields one spoofed and one real response; only the real
    // response may be counted.
    assert_eq!(stats.total_requests, 10);
    assert_eq!(stats.successful_requests, 10);
    assert_eq!(stats.failed_requests, 0);
}

#[tokio::test]
async fn test_benchmark_counts_unanswered_requests_as_failed() {
    let server_addr = spawn_dropping_server().await;

    let config = BenchConfig {
        server_addrs: vec![server_addr],
        target_qps: 500,
        duration: None,
        request_count: Some(10),
        fingerprint: false,
        auth: BenchAuth::disabled(),
    };

    let stats = bench::run_benchmark(&config).await.unwrap();

    assert_eq!(stats.total_requests, 10);
    assert_eq!(stats.successful_requests, 0);
    assert_eq!(stats.failed_requests, 10);
}

#[tokio::test]
async fn test_benchmark_short_term_auth_succeeds() {
    let (server_addr, accepted) = spawn_short_term_server("alice", "s3cret", "s3cret").await;

    let config = BenchConfig {
        server_addrs: vec![server_addr],
        target_qps: 200,
        duration: None,
        request_count: Some(10),
        fingerprint: false,
        auth: BenchAuth::short_term("alice", "s3cret"),
    };

    let stats = bench::run_benchmark(&config).await.unwrap();

    assert_eq!(stats.total_requests, 10);
    assert_eq!(stats.successful_requests, 10);
    assert_eq!(stats.failed_requests, 0);
    assert_eq!(accepted.load(Ordering::Relaxed), 10);
}

#[tokio::test]
async fn test_benchmark_short_term_wrong_password_fails() {
    let (server_addr, accepted) = spawn_short_term_server("alice", "s3cret", "s3cret").await;

    let config = BenchConfig {
        server_addrs: vec![server_addr],
        target_qps: 200,
        duration: None,
        request_count: Some(10),
        fingerprint: false,
        auth: BenchAuth::short_term("alice", "wrong"),
    };

    let stats = bench::run_benchmark(&config).await.unwrap();

    assert_eq!(stats.successful_requests, 0);
    assert_eq!(stats.failed_requests, 10);
    assert_eq!(accepted.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn test_benchmark_rejects_invalid_response_integrity() {
    // The server accepts our request but signs the response with another key.
    let (server_addr, accepted) = spawn_short_term_server("alice", "s3cret", "other").await;

    let config = BenchConfig {
        server_addrs: vec![server_addr],
        target_qps: 200,
        duration: None,
        request_count: Some(10),
        fingerprint: false,
        auth: BenchAuth::short_term("alice", "s3cret"),
    };

    let stats = bench::run_benchmark(&config).await.unwrap();

    assert_eq!(stats.successful_requests, 0);
    assert_eq!(stats.failed_requests, 10);
    assert_eq!(accepted.load(Ordering::Relaxed), 10);
}

#[tokio::test]
async fn test_benchmark_long_term_bootstraps_challenge() {
    let (server_addr, accepted) =
        spawn_long_term_server("alice", "s3cret", "example.org", "nonce-1").await;

    let config = BenchConfig {
        server_addrs: vec![server_addr],
        target_qps: 200,
        duration: None,
        request_count: Some(10),
        fingerprint: false,
        auth: BenchAuth::long_term("alice", "s3cret"),
    };

    let stats = bench::run_benchmark(&config).await.unwrap();

    assert_eq!(stats.total_requests, 10);
    assert_eq!(stats.successful_requests, 10);
    assert_eq!(stats.failed_requests, 0);
    assert_eq!(accepted.load(Ordering::Relaxed), 10);
}

#[tokio::test]
async fn test_benchmark_long_term_realm_override_matches_server() {
    let (server_addr, accepted) =
        spawn_long_term_server("alice", "s3cret", "example.org", "nonce-1").await;

    let config = BenchConfig {
        server_addrs: vec![server_addr],
        target_qps: 200,
        duration: None,
        request_count: Some(10),
        fingerprint: false,
        auth: BenchAuth::long_term_with_realm("alice", "s3cret", "example.org"),
    };

    let stats = bench::run_benchmark(&config).await.unwrap();

    assert_eq!(stats.successful_requests, 10);
    assert_eq!(stats.failed_requests, 0);
    assert_eq!(accepted.load(Ordering::Relaxed), 10);
}

#[tokio::test]
async fn test_benchmark_long_term_realm_override_mismatch_fails() {
    let (server_addr, accepted) =
        spawn_long_term_server("alice", "s3cret", "example.org", "nonce-1").await;

    let config = BenchConfig {
        server_addrs: vec![server_addr],
        target_qps: 200,
        duration: None,
        request_count: Some(10),
        fingerprint: false,
        auth: BenchAuth::long_term_with_realm("alice", "s3cret", "other.example"),
    };

    let stats = bench::run_benchmark(&config).await.unwrap();

    assert_eq!(stats.successful_requests, 0);
    assert_eq!(stats.failed_requests, 10);
    assert_eq!(accepted.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn test_benchmark_long_term_wrong_password_fails() {
    let (server_addr, accepted) =
        spawn_long_term_server("alice", "s3cret", "example.org", "nonce-1").await;

    let config = BenchConfig {
        server_addrs: vec![server_addr],
        target_qps: 200,
        duration: None,
        request_count: Some(10),
        fingerprint: false,
        auth: BenchAuth::long_term("alice", "wrong"),
    };

    let stats = bench::run_benchmark(&config).await.unwrap();

    assert_eq!(stats.successful_requests, 0);
    assert_eq!(stats.failed_requests, 10);
    assert_eq!(accepted.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn test_benchmark_long_term_requires_challenge() {
    // The mock never challenges, so authentication cannot proceed.
    let server_addr = spawn_mock_server().await;

    let config = BenchConfig {
        server_addrs: vec![server_addr],
        target_qps: 200,
        duration: None,
        request_count: Some(10),
        fingerprint: false,
        auth: BenchAuth::long_term("alice", "s3cret"),
    };

    let result = bench::run_benchmark(&config).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_benchmark_long_term_refreshes_stale_nonce() {
    let (server_addr, accepted, stale) =
        spawn_rotating_nonce_server("alice", "s3cret", "example.org", "nonce", 3).await;

    let config = BenchConfig {
        server_addrs: vec![server_addr],
        target_qps: 50,
        duration: None,
        request_count: Some(12),
        fingerprint: false,
        auth: BenchAuth::long_term("alice", "s3cret"),
    };

    let stats = bench::run_benchmark(&config).await.unwrap();

    assert_eq!(stats.total_requests, 12);
    // Three nonce rotations over 12 requests: nine successes, three 438s.
    assert_eq!(stale.load(Ordering::Relaxed), 3);
    assert_eq!(accepted.load(Ordering::Relaxed), 9);
    assert_eq!(stats.successful_requests, 9);
    assert_eq!(stats.failed_requests, 3);
}

#[tokio::test]
async fn test_benchmark_long_term_realm_override_survives_nonce_refresh() {
    let (server_addr, accepted, stale) =
        spawn_rotating_nonce_server("alice", "s3cret", "example.org", "nonce", 3).await;

    let config = BenchConfig {
        server_addrs: vec![server_addr],
        target_qps: 50,
        duration: None,
        request_count: Some(12),
        fingerprint: false,
        auth: BenchAuth::long_term_with_realm("alice", "s3cret", "example.org"),
    };

    let stats = bench::run_benchmark(&config).await.unwrap();

    assert_eq!(stats.total_requests, 12);
    assert_eq!(stale.load(Ordering::Relaxed), 3);
    assert_eq!(accepted.load(Ordering::Relaxed), 9);
    assert_eq!(stats.successful_requests, 9);
    assert_eq!(stats.failed_requests, 3);
}

#[test]
fn test_stats_calculation() {
    let latencies = vec![
        Duration::from_millis(1),
        Duration::from_millis(2),
        Duration::from_millis(3),
        Duration::from_millis(4),
        Duration::from_millis(5),
    ];

    let stats = BenchmarkStats::calculate(1000, 100, 95, 5, &latencies, Duration::from_secs(1));

    assert_eq!(stats.target_qps, 1000);
    assert_eq!(stats.total_requests, 100);
    assert_eq!(stats.successful_requests, 95);
    assert_eq!(stats.failed_requests, 5);
    assert!((stats.success_rate - 95.0).abs() < 0.01);
    assert!((stats.actual_qps - 100.0).abs() < 0.01);
}

#[test]
fn test_latency_collector_percentiles() {
    let mut collector = LatencyCollector::new();
    for i in 1..=100 {
        collector.record_latency(Duration::from_millis(i));
    }

    let (p50, p95, p99) = collector.calculate_percentiles();

    assert!(p50.as_millis() >= 49 && p50.as_millis() <= 51);
    assert!(p95.as_millis() >= 94 && p95.as_millis() <= 96);
    assert!(p99.as_millis() >= 98 && p99.as_millis() <= 100);
}

#[test]
fn test_latency_collector_empty() {
    let collector = LatencyCollector::new();
    let (p50, p95, p99) = collector.calculate_percentiles();
    assert_eq!(p50, Duration::ZERO);
    assert_eq!(p95, Duration::ZERO);
    assert_eq!(p99, Duration::ZERO);
}

#[test]
fn test_benchmark_stats_zero_requests() {
    let stats = BenchmarkStats::calculate(1000, 0, 0, 0, &[], Duration::from_secs(1));

    assert_eq!(stats.total_requests, 0);
    assert_eq!(stats.successful_requests, 0);
    assert_eq!(stats.failed_requests, 0);
    assert_eq!(stats.success_rate, 0.0);
    assert_eq!(stats.actual_qps, 0.0);
}

#[test]
fn test_benchmark_stats_high_success_rate() {
    let stats = BenchmarkStats::calculate(
        10000,
        10000,
        10000,
        0,
        &[Duration::from_millis(1)],
        Duration::from_secs(1),
    );

    assert!((stats.success_rate - 100.0).abs() < 0.01);
    assert_eq!(stats.failed_requests, 0);
}

#[test]
fn test_benchmark_stats_low_success_rate() {
    let stats = BenchmarkStats::calculate(
        10000,
        1000,
        100,
        900,
        &[Duration::from_millis(1)],
        Duration::from_secs(1),
    );

    assert!((stats.success_rate - 10.0).abs() < 0.01);
    assert_eq!(stats.failed_requests, 900);
}

#[test]
fn test_build_request_without_fingerprint() {
    let (message, bytes) = bench::build_request([0x42; 12], false, &disabled_auth(), None).unwrap();

    assert_eq!(
        message.attribute_count(StunAttributeType::Fingerprint as u16),
        0
    );
    assert!(!StunMessage::verify_message_fingerprint(&bytes));
}

#[test]
fn test_build_request_with_terminal_fingerprint() {
    let (message, bytes) = bench::build_request([0x42; 12], true, &disabled_auth(), None).unwrap();

    assert_eq!(
        message.attribute_count(StunAttributeType::Fingerprint as u16),
        1
    );
    assert_eq!(
        message.attributes.last().unwrap().raw_type,
        StunAttributeType::Fingerprint as u16
    );
    assert!(StunMessage::verify_message_fingerprint(&bytes));
}

#[test]
fn test_build_request_short_term_signs_with_password_key() {
    let auth = BenchAuth::short_term("alice", "s3cret").prepare().unwrap();
    let (message, bytes) = bench::build_request([0x42; 12], false, &auth, None).unwrap();
    let key = integrity::short_term_key("s3cret").unwrap();

    assert_eq!(
        message.attribute_count(StunAttributeType::MessageIntegrity as u16),
        1
    );
    assert!(has_text_attribute(
        &message,
        StunAttributeType::Username,
        "alice"
    ));
    assert!(StunMessage::verify_message_integrity(&bytes, &key));
}

#[test]
fn test_build_request_long_term_uses_challenge() {
    let auth = BenchAuth::long_term("alice", "s3cret").prepare().unwrap();
    let challenge = bench::Challenge {
        realm: "example.org".to_string(),
        nonce: "nonce-1".to_string(),
    };
    let (message, bytes) =
        bench::build_request([0x42; 12], false, &auth, Some(&challenge)).unwrap();
    let key = integrity::long_term_key("alice", "example.org", "s3cret").unwrap();

    assert!(has_text_attribute(
        &message,
        StunAttributeType::Username,
        "alice"
    ));
    assert!(has_text_attribute(
        &message,
        StunAttributeType::Realm,
        "example.org"
    ));
    assert!(has_text_attribute(
        &message,
        StunAttributeType::Nonce,
        "nonce-1"
    ));
    assert!(StunMessage::verify_message_integrity(&bytes, &key));
}

#[test]
fn test_build_request_long_term_without_challenge_is_unsigned() {
    let auth = BenchAuth::long_term("alice", "s3cret").prepare().unwrap();
    let (message, _bytes) = bench::build_request([0x42; 12], false, &auth, None).unwrap();

    assert_eq!(
        message.attribute_count(StunAttributeType::MessageIntegrity as u16),
        0
    );
    assert_eq!(
        message.attribute_count(StunAttributeType::Username as u16),
        0
    );
}

#[test]
fn test_build_request_integrity_then_fingerprint() {
    let auth = BenchAuth::short_term("alice", "s3cret").prepare().unwrap();
    let (message, bytes) = bench::build_request([0x42; 12], true, &auth, None).unwrap();
    let key = integrity::short_term_key("s3cret").unwrap();

    let types: Vec<u16> = message.attributes.iter().map(|a| a.raw_type).collect();
    let mi = types
        .iter()
        .position(|t| *t == StunAttributeType::MessageIntegrity as u16)
        .expect("MESSAGE-INTEGRITY present");
    assert_eq!(mi + 1, types.len() - 1);
    assert_eq!(
        *types.last().unwrap(),
        StunAttributeType::Fingerprint as u16
    );
    assert!(StunMessage::verify_message_integrity(&bytes, &key));
    assert!(StunMessage::verify_message_fingerprint(&bytes));
}

#[test]
fn test_bench_auth_requires_credentials() {
    assert!(BenchAuth::short_term("", "s3cret").prepare().is_err());
    assert!(BenchAuth::short_term("alice", "").prepare().is_err());
    assert!(BenchAuth::long_term("", "s3cret").prepare().is_err());
    assert!(BenchAuth::long_term("alice", "").prepare().is_err());
}

#[test]
fn test_bench_auth_disabled_has_no_credentials() {
    let auth = disabled_auth();
    assert_eq!(auth.mode(), AuthMode::Disabled);
    assert!(bench::signing_key(&auth, None).unwrap().is_none());
}

#[tokio::test]
async fn test_benchmark_with_fingerprint_succeeds() {
    let socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let addr = socket.local_addr().unwrap();
    let socket = Arc::new(socket);

    let verified = Arc::new(AtomicUsize::new(0));
    let counter = verified.clone();
    let verifier = tokio::spawn(async move {
        let mut buf = vec![0u8; 65536];
        loop {
            let (len, from) = match socket.recv_from(&mut buf).await {
                Ok(r) => r,
                Err(_) => break,
            };
            if !StunMessage::verify_message_fingerprint(&buf[..len]) {
                continue;
            }
            if let Ok(request) = StunMessage::decode(&buf[..len]) {
                let mut response =
                    StunMessage::new(StunMessageType::Response, request.transaction_id);
                let mapped = MappedAddress::new(from.ip(), from.port());
                let xor_value = mapped.encode_xor(response.magic_cookie, &response.transaction_id);
                response.add_attribute(StunAttribute::new(
                    StunAttributeType::XorMappedAddress,
                    xor_value,
                ));
                let _ = socket.send_to(&response.encode(), from).await;
                counter.fetch_add(1, Ordering::Relaxed);
            }
        }
    });

    let config = BenchConfig {
        server_addrs: vec![addr],
        target_qps: 200,
        duration: None,
        request_count: Some(50),
        fingerprint: true,
        auth: BenchAuth::disabled(),
    };
    let stats = bench::run_benchmark(&config).await.unwrap();

    verifier.abort();
    assert!(stats.successful_requests > 0);
    assert!(verified.load(Ordering::Relaxed) > 0);
}
