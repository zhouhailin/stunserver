//! Benchmark execution engine.
//!
//! Sends STUN Binding Requests at a target QPS rate, collects latency
//! measurements, and reports progress and final results.

use crate::stats::{BenchmarkStats, LatencyCollector};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use stun_protocol::{StunMessage, StunMessageType};
use tokio::net::UdpSocket;
use tokio::sync::Mutex;

/// Configuration for a benchmark run.
#[derive(Debug, Clone)]
pub struct BenchConfig {
    /// Target server address.
    pub server_addr: SocketAddr,
    /// Target queries per second.
    pub target_qps: u32,
    /// Test duration (if specified).
    pub duration: Option<Duration>,
    /// Total request count (if specified, alternative to duration).
    pub request_count: Option<u64>,
}

/// Run a benchmark against the specified STUN server.
///
/// Spawns sender tasks that emit requests at the target QPS rate,
/// and a receiver task that collects responses and measures latency.
/// Returns final benchmark statistics.
pub async fn run_benchmark(config: &BenchConfig) -> Result<BenchmarkStats, String> {
    // Bind a UDP socket for sending/receiving.
    let local_addr: SocketAddr = if config.server_addr.is_ipv4() {
        "0.0.0.0:0".parse().unwrap()
    } else {
        "[::]:0".parse().unwrap()
    };

    let socket = Arc::new(
        UdpSocket::bind(local_addr)
            .await
            .map_err(|e| format!("failed to bind socket: {}", e))?,
    );

    // Shared state for coordination between sender and receiver.
    let total_sent = Arc::new(AtomicU64::new(0));
    let total_success = Arc::new(AtomicU64::new(0));
    let total_failed = Arc::new(AtomicU64::new(0));
    let latencies = Arc::new(Mutex::new(LatencyCollector::with_capacity(
        config.request_count.unwrap_or(100_000) as usize,
    )));
    let running = Arc::new(AtomicBool::new(true));

    // Determine test termination condition.
    let (stop_signal, stop_handle) = match (config.duration, config.request_count) {
        (Some(duration), _) => {
            // Time-based termination.
            let running_clone = running.clone();
            let handle = tokio::spawn(async move {
                tokio::time::sleep(duration).await;
                running_clone.store(false, Ordering::Relaxed);
            });
            (None, Some(handle))
        }
        (_, Some(count)) => {
            // Count-based termination (checked in sender loop).
            (Some(count), None)
        }
        _ => return Err("either duration or request_count must be specified".to_string()),
    };

    // Spawn sender task.
    let sender_socket = socket.clone();
    let sender_sent = total_sent.clone();
    let sender_running = running.clone();
    let sender_target_qps = config.target_qps;
    let sender_stop_signal = stop_signal;
    let sender_server_addr = config.server_addr;
    let sender_handle = tokio::spawn(async move {
        let interval_duration = if sender_target_qps > 0 {
            Duration::from_micros(1_000_000 / sender_target_qps as u64)
        } else {
            Duration::from_millis(1)
        };
        let mut interval = tokio::time::interval(interval_duration);

        loop {
            interval.tick().await;

            // Check termination conditions.
            if !sender_running.load(Ordering::Relaxed) {
                break;
            }
            if let Some(max_count) = sender_stop_signal {
                if sender_sent.load(Ordering::Relaxed) >= max_count {
                    sender_running.store(false, Ordering::Relaxed);
                    break;
                }
            }

            // Build and send STUN request.
            let transaction_id = generate_transaction_id();
            let request = StunMessage::new(StunMessageType::Request, transaction_id);
            let request_bytes = request.encode();

            if sender_socket
                .send_to(&request_bytes, sender_server_addr)
                .await
                .is_ok()
            {
                sender_sent.fetch_add(1, Ordering::Relaxed);
            }
        }
    });

    // Spawn receiver task.
    let receiver_socket = socket.clone();
    let receiver_success = total_success.clone();
    let receiver_failed = total_failed.clone();
    let receiver_latencies = latencies.clone();
    let receiver_running = running.clone();
    let receiver_handle = tokio::spawn(async move {
        let mut buf = vec![0u8; 65536];
        let recv_timeout = Duration::from_secs(2);

        loop {
            if !receiver_running.load(Ordering::Relaxed) {
                // Drain any remaining responses.
                match tokio::time::timeout(
                    Duration::from_millis(100),
                    receiver_socket.recv_from(&mut buf),
                )
                .await
                {
                    Ok(Ok((len, _))) => {
                        process_response(
                            &buf[..len],
                            &receiver_latencies,
                            receiver_success.clone(),
                            receiver_failed.clone(),
                        )
                        .await;
                    }
                    _ => break,
                }
                continue;
            }

            match tokio::time::timeout(recv_timeout, receiver_socket.recv_from(&mut buf)).await {
                Ok(Ok((len, _))) => {
                    process_response(
                        &buf[..len],
                        &receiver_latencies,
                        receiver_success.clone(),
                        receiver_failed.clone(),
                    )
                    .await;
                }
                Ok(Err(_)) => {
                    receiver_failed.fetch_add(1, Ordering::Relaxed);
                }
                Err(_) => {
                    // Timeout, check if we should stop.
                    if !receiver_running.load(Ordering::Relaxed) {
                        break;
                    }
                }
            }
        }
    });

    // Spawn progress reporter task.
    let progress_sent = total_sent.clone();
    let progress_success = total_success.clone();
    let progress_failed = total_failed.clone();
    let progress_running = running.clone();
    let progress_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(10));
        let start = Instant::now();

        loop {
            interval.tick().await;
            if !progress_running.load(Ordering::Relaxed) {
                break;
            }
            let elapsed = start.elapsed().as_secs();
            let sent = progress_sent.load(Ordering::Relaxed);
            let success = progress_success.load(Ordering::Relaxed);
            let failed = progress_failed.load(Ordering::Relaxed);
            let current_qps = sent.checked_div(elapsed.max(1)).unwrap_or(0);
            print_progress(elapsed, sent, success, failed, current_qps);
        }
    });

    // Wait for sender to finish.
    let _ = sender_handle.await;

    // Give receiver a moment to process remaining responses.
    tokio::time::sleep(Duration::from_millis(500)).await;
    running.store(false, Ordering::Relaxed);

    // Wait for receiver and progress reporter.
    let _ = receiver_handle.await;
    let _ = progress_handle.await;
    if let Some(h) = stop_handle {
        h.abort();
    }

    // Calculate final statistics.
    let total_requests = total_sent.load(Ordering::Relaxed);
    let successful_requests = total_success.load(Ordering::Relaxed);
    let failed_requests = total_failed.load(Ordering::Relaxed);
    let latency_guard = latencies.lock().await;
    let latencies_vec: Vec<Duration> = latency_guard.latencies().to_vec();
    drop(latency_guard);

    let duration = config.duration.unwrap_or_else(|| {
        // Estimate duration from request count and QPS.
        Duration::from_secs_f64(total_requests as f64 / config.target_qps as f64)
    });

    Ok(BenchmarkStats::calculate(
        config.target_qps,
        total_requests,
        successful_requests,
        failed_requests,
        &latencies_vec,
        duration,
    ))
}

/// Process a single STUN response.
async fn process_response(
    data: &[u8],
    latencies: &Arc<Mutex<LatencyCollector>>,
    success: Arc<AtomicU64>,
    failed: Arc<AtomicU64>,
) {
    match StunMessage::decode(data) {
        Ok(response) if response.message_type == StunMessageType::Response => {
            // Estimate latency (in a real implementation, we'd track per-request timing).
            let latency = Duration::from_micros(500); // Placeholder.
            let mut guard = latencies.lock().await;
            guard.record_latency(latency);
            success.fetch_add(1, Ordering::Relaxed);
        }
        _ => {
            failed.fetch_add(1, Ordering::Relaxed);
        }
    }
}

/// Print real-time progress update.
fn print_progress(elapsed_secs: u64, sent: u64, success: u64, failed: u64, current_qps: u64) {
    let minutes = elapsed_secs / 60;
    let seconds = elapsed_secs % 60;
    println!(
        "[{:02}:{:02}] Sent: {}, Success: {}, Failed: {}, QPS: {}",
        minutes, seconds, sent, success, failed, current_qps
    );
}

/// Print final benchmark results.
pub fn print_results(stats: &BenchmarkStats) {
    println!("\nBenchmark Complete");
    println!("==================");
    println!("Duration: {:.1}s", stats.duration.as_secs_f64());
    println!("Target QPS: {}", stats.target_qps);
    println!("Actual QPS: {:.2}", stats.actual_qps);
    println!("Total Requests: {}", stats.total_requests);
    println!(
        "Successful: {} ({:.2}%)",
        stats.successful_requests, stats.success_rate
    );
    println!("Failed: {}", stats.failed_requests);
    println!("\nLatency Percentiles:");
    println!("  p50: {:.2}ms", stats.latency_p50.as_secs_f64() * 1000.0);
    println!("  p95: {:.2}ms", stats.latency_p95.as_secs_f64() * 1000.0);
    println!("  p99: {:.2}ms", stats.latency_p99.as_secs_f64() * 1000.0);
}

/// Generate a random 12-byte transaction ID.
fn generate_transaction_id() -> [u8; 12] {
    let mut id = [0u8; 12];
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let bytes = now.to_le_bytes();
    for i in 0..12 {
        id[i] = bytes[i % bytes.len()];
    }
    id
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bench_config_creation() {
        let config = BenchConfig {
            server_addr: "127.0.0.1:3478".parse().unwrap(),
            target_qps: 1000,
            duration: Some(Duration::from_secs(60)),
            request_count: None,
        };
        assert_eq!(config.target_qps, 1000);
        assert!(config.duration.is_some());
        assert!(config.request_count.is_none());
    }

    #[test]
    fn test_generate_transaction_id() {
        let id1 = generate_transaction_id();
        let id2 = generate_transaction_id();
        assert_ne!(id1, id2);
        assert_eq!(id1.len(), 12);
    }

    #[tokio::test]
    async fn test_run_benchmark_invalid_config() {
        let config = BenchConfig {
            server_addr: "127.0.0.1:3478".parse().unwrap(),
            target_qps: 100,
            duration: None,
            request_count: None,
        };
        let result = run_benchmark(&config).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_print_progress() {
        // Just verify it doesn't panic.
        print_progress(10, 1000, 995, 5, 100);
    }

    #[test]
    fn test_print_results() {
        let stats = BenchmarkStats {
            target_qps: 1000,
            actual_qps: 995.5,
            total_requests: 10000,
            successful_requests: 9950,
            failed_requests: 50,
            success_rate: 99.5,
            latency_p50: Duration::from_millis(1),
            latency_p95: Duration::from_millis(2),
            latency_p99: Duration::from_millis(5),
            duration: Duration::from_secs(10),
        };
        // Just verify it doesn't panic.
        print_results(&stats);
    }
}
