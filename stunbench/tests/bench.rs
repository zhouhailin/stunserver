//! Integration tests for STUN Benchmark tool.
//!
//! Tests cover statistics calculation, latency percentiles, and
//! end-to-end benchmark execution against a mock STUN server.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use stun_protocol::{
    MappedAddress, StunAttribute, StunAttributeType, StunMessage, StunMessageType,
};
use stunbench::bench::{self, BenchConfig};
use stunbench::stats::{BenchmarkStats, LatencyCollector};
use tokio::net::UdpSocket;

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

#[tokio::test]
async fn test_benchmark_short_duration() {
    let server_addr = spawn_mock_server().await;

    let config = BenchConfig {
        server_addr,
        target_qps: 100,
        duration: Some(Duration::from_secs(2)),
        request_count: None,
    };

    let stats = bench::run_benchmark(&config).await.unwrap();

    assert!(stats.total_requests > 0);
    assert!(stats.successful_requests > 0);
    assert_eq!(stats.target_qps, 100);
    assert!(stats.duration.as_secs() >= 2);
}

#[tokio::test]
async fn test_benchmark_request_count() {
    let server_addr = spawn_mock_server().await;

    let config = BenchConfig {
        server_addr,
        target_qps: 500,
        duration: None,
        request_count: Some(100),
    };

    let stats = bench::run_benchmark(&config).await.unwrap();

    // Should have sent approximately 100 requests.
    assert!(stats.total_requests >= 90 && stats.total_requests <= 110);
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
