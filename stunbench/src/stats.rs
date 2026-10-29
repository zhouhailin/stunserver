//! Benchmark statistics collection and calculation.
//!
//! Tracks request counts, latency measurements, and computes
//! percentile statistics for benchmark results.

use std::time::Duration;

/// Final benchmark results.
#[derive(Debug, Clone)]
pub struct BenchmarkStats {
    /// Target queries per second.
    pub target_qps: u32,
    /// Actual achieved queries per second.
    pub actual_qps: f64,
    /// Total number of requests sent.
    pub total_requests: u64,
    /// Number of successful responses received.
    pub successful_requests: u64,
    /// Number of failed or timed-out requests.
    pub failed_requests: u64,
    /// Success rate as a percentage (0.0-100.0).
    pub success_rate: f64,
    /// 50th percentile latency.
    pub latency_p50: Duration,
    /// 95th percentile latency.
    pub latency_p95: Duration,
    /// 99th percentile latency.
    pub latency_p99: Duration,
    /// Total test duration.
    pub duration: Duration,
}

impl BenchmarkStats {
    /// Calculate statistics from raw benchmark data.
    pub fn calculate(
        target_qps: u32,
        total_requests: u64,
        successful_requests: u64,
        failed_requests: u64,
        latencies: &[Duration],
        duration: Duration,
    ) -> Self {
        let actual_qps = if duration.as_secs_f64() > 0.0 {
            total_requests as f64 / duration.as_secs_f64()
        } else {
            0.0
        };

        let success_rate = if total_requests > 0 {
            (successful_requests as f64 / total_requests as f64) * 100.0
        } else {
            0.0
        };

        let (p50, p95, p99) = calculate_percentiles(latencies);

        Self {
            target_qps,
            actual_qps,
            total_requests,
            successful_requests,
            failed_requests,
            success_rate,
            latency_p50: p50,
            latency_p95: p95,
            latency_p99: p99,
            duration,
        }
    }
}

/// Collects latency measurements during a benchmark run.
pub struct LatencyCollector {
    latencies: Vec<Duration>,
}

impl LatencyCollector {
    /// Create a new latency collector.
    pub fn new() -> Self {
        Self {
            latencies: Vec::new(),
        }
    }

    /// Create a new latency collector with pre-allocated capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            latencies: Vec::with_capacity(capacity),
        }
    }

    /// Record a latency measurement.
    pub fn record_latency(&mut self, latency: Duration) {
        self.latencies.push(latency);
    }

    /// Get all recorded latencies.
    pub fn latencies(&self) -> &[Duration] {
        &self.latencies
    }

    /// Calculate percentiles from recorded latencies.
    pub fn calculate_percentiles(&self) -> (Duration, Duration, Duration) {
        calculate_percentiles(&self.latencies)
    }
}

impl Default for LatencyCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// Calculate p50, p95, p99 percentiles from a slice of latencies.
///
/// Uses a simple sorted-vector approach. For very large datasets,
/// a more efficient algorithm (e.g., HDR histogram) could be used.
fn calculate_percentiles(latencies: &[Duration]) -> (Duration, Duration, Duration) {
    if latencies.is_empty() {
        return (Duration::ZERO, Duration::ZERO, Duration::ZERO);
    }

    let mut sorted: Vec<Duration> = latencies.to_vec();
    sorted.sort();

    let len = sorted.len();
    let p50 = sorted[percentile_index(len, 50)];
    let p95 = sorted[percentile_index(len, 95)];
    let p99 = sorted[percentile_index(len, 99)];

    (p50, p95, p99)
}

/// Calculate the index for a given percentile.
fn percentile_index(len: usize, percentile: u32) -> usize {
    if len == 0 {
        return 0;
    }
    let idx = (len as f64 * percentile as f64 / 100.0).ceil() as usize;
    idx.saturating_sub(1).min(len - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_latency_collector_new() {
        let collector = LatencyCollector::new();
        assert!(collector.latencies().is_empty());
    }

    #[test]
    fn test_latency_collector_record() {
        let mut collector = LatencyCollector::new();
        collector.record_latency(Duration::from_millis(10));
        collector.record_latency(Duration::from_millis(20));
        assert_eq!(collector.latencies().len(), 2);
    }

    #[test]
    fn test_latency_collector_with_capacity() {
        let collector = LatencyCollector::with_capacity(1000);
        assert!(collector.latencies().is_empty());
    }

    #[test]
    fn test_calculate_percentiles_empty() {
        let (p50, p95, p99) = calculate_percentiles(&[]);
        assert_eq!(p50, Duration::ZERO);
        assert_eq!(p95, Duration::ZERO);
        assert_eq!(p99, Duration::ZERO);
    }

    #[test]
    fn test_calculate_percentiles_single() {
        let latencies = vec![Duration::from_millis(10)];
        let (p50, p95, p99) = calculate_percentiles(&latencies);
        assert_eq!(p50, Duration::from_millis(10));
        assert_eq!(p95, Duration::from_millis(10));
        assert_eq!(p99, Duration::from_millis(10));
    }

    #[test]
    fn test_calculate_percentiles_multiple() {
        let mut latencies = Vec::new();
        for i in 1..=100 {
            latencies.push(Duration::from_millis(i));
        }
        let (p50, p95, p99) = calculate_percentiles(&latencies);
        // p50 should be around 50ms, p95 around 95ms, p99 around 99ms.
        assert!(p50.as_millis() >= 49 && p50.as_millis() <= 51);
        assert!(p95.as_millis() >= 94 && p95.as_millis() <= 96);
        assert!(p99.as_millis() >= 98 && p99.as_millis() <= 100);
    }

    #[test]
    fn test_calculate_percentiles_unsorted() {
        let latencies = vec![
            Duration::from_millis(100),
            Duration::from_millis(10),
            Duration::from_millis(50),
        ];
        let (p50, _, _) = calculate_percentiles(&latencies);
        // Median of [10, 50, 100] is 50.
        assert_eq!(p50, Duration::from_millis(50));
    }

    #[test]
    fn test_benchmark_stats_calculate() {
        let latencies = vec![
            Duration::from_millis(10),
            Duration::from_millis(20),
            Duration::from_millis(30),
        ];
        let stats = BenchmarkStats::calculate(
            1000, // target_qps
            100,  // total_requests
            95,   // successful_requests
            5,    // failed_requests
            &latencies,
            Duration::from_secs(1), // duration
        );

        assert_eq!(stats.target_qps, 1000);
        assert_eq!(stats.total_requests, 100);
        assert_eq!(stats.successful_requests, 95);
        assert_eq!(stats.failed_requests, 5);
        assert!((stats.success_rate - 95.0).abs() < 0.01);
        assert!((stats.actual_qps - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_benchmark_stats_zero_duration() {
        let stats = BenchmarkStats::calculate(1000, 0, 0, 0, &[], Duration::ZERO);
        assert_eq!(stats.actual_qps, 0.0);
        assert_eq!(stats.success_rate, 0.0);
    }

    #[test]
    fn test_percentile_index() {
        assert_eq!(percentile_index(100, 50), 49);
        assert_eq!(percentile_index(100, 95), 94);
        assert_eq!(percentile_index(100, 99), 98);
        assert_eq!(percentile_index(10, 50), 4);
        assert_eq!(percentile_index(1, 50), 0);
        assert_eq!(percentile_index(0, 50), 0);
    }
}
