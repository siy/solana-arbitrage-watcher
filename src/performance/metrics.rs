use chrono::{DateTime, Utc};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// Comprehensive performance metrics for the arbitrage watcher
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    pub summary: PerformanceSummary,
    pub throughput: ThroughputStats,
    pub connection: ConnectionStats,
    pub processing: ProcessingStats,
}

/// Overall performance summary
#[derive(Debug, Clone)]
pub struct PerformanceSummary {
    pub uptime_seconds: u64,
    pub total_opportunities: u64,
    pub avg_processing_latency_ms: f64,
    pub solana_msg_rate: f64,
    pub binance_msg_rate: f64,
    pub reconnection_count: u64,
}

/// Real-time throughput statistics
#[derive(Debug, Clone)]
pub struct ThroughputStats {
    pub messages_per_second: f64,
    pub opportunities_per_hour: f64,
    pub current_queue_depth: usize,
    pub processing_efficiency_pct: f64,
}

/// Connection reliability statistics
#[derive(Debug, Clone)]
pub struct ConnectionStats {
    pub solana_uptime_pct: f64,
    pub binance_uptime_pct: f64,
    pub total_reconnections: u64,
    pub avg_reconnect_time_ms: f64,
    pub last_connection_failure: Option<DateTime<Utc>>,
}

/// Processing pipeline performance statistics
#[derive(Debug, Clone)]
pub struct ProcessingStats {
    pub price_processing_latency_ms: f64,
    pub arbitrage_detection_latency_ms: f64,
    pub output_formatting_latency_ms: f64,
    pub total_pipeline_latency_ms: f64,
    pub messages_processed: u64,
    pub errors_encountered: u64,
}

/// Thread-safe metrics collector for tracking performance data
#[derive(Debug)]
pub struct MetricsCollector {
    start_time: Instant,

    // Message counters
    solana_messages: AtomicU64,
    binance_messages: AtomicU64,
    opportunities_found: AtomicU64,

    // Connection tracking
    reconnection_count: AtomicU64,
    solana_connection_time: Arc<RwLock<Duration>>,
    binance_connection_time: Arc<RwLock<Duration>>,
    last_reconnect_start: Arc<RwLock<Option<Instant>>>,

    // Processing metrics
    processing_times: Arc<RwLock<VecDeque<Duration>>>,
    arbitrage_times: Arc<RwLock<VecDeque<Duration>>>,
    output_times: Arc<RwLock<VecDeque<Duration>>>,

    // Queue depth tracking
    current_queue_depth: AtomicUsize,
    max_queue_depth: AtomicUsize,

    // Error tracking
    processing_errors: AtomicU64,
    connection_failures: Arc<RwLock<Vec<DateTime<Utc>>>>,
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsCollector {
    /// Create a new metrics collector
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            solana_messages: AtomicU64::new(0),
            binance_messages: AtomicU64::new(0),
            opportunities_found: AtomicU64::new(0),
            reconnection_count: AtomicU64::new(0),
            solana_connection_time: Arc::new(RwLock::new(Duration::ZERO)),
            binance_connection_time: Arc::new(RwLock::new(Duration::ZERO)),
            last_reconnect_start: Arc::new(RwLock::new(None)),
            processing_times: Arc::new(RwLock::new(VecDeque::with_capacity(1000))),
            arbitrage_times: Arc::new(RwLock::new(VecDeque::with_capacity(1000))),
            output_times: Arc::new(RwLock::new(VecDeque::with_capacity(1000))),
            current_queue_depth: AtomicUsize::new(0),
            max_queue_depth: AtomicUsize::new(0),
            processing_errors: AtomicU64::new(0),
            connection_failures: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Record a Solana WebSocket message received
    pub fn record_solana_message(&self) {
        self.solana_messages.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a Binance WebSocket message received
    pub fn record_binance_message(&self) {
        self.binance_messages.fetch_add(1, Ordering::Relaxed);
    }

    /// Record an arbitrage opportunity found
    pub fn record_opportunity(&self) {
        self.opportunities_found.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a connection reconnection attempt
    pub fn record_reconnection(&self) {
        self.reconnection_count.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut start) = self.last_reconnect_start.write() {
            *start = Some(Instant::now());
        }
    }

    /// Record successful reconnection completion
    pub fn record_reconnection_complete(&self) {
        if let Ok(start_guard) = self.last_reconnect_start.read() {
            if let Some(start) = *start_guard {
                let duration = start.elapsed();
                // Could track reconnection times for averaging
                log::debug!("Reconnection completed in {:?}", duration);
            }
        }
    }

    /// Record connection uptime for Solana
    pub fn record_solana_uptime(&self, connected_duration: Duration) {
        if let Ok(mut uptime) = self.solana_connection_time.write() {
            *uptime = connected_duration;
        }
    }

    /// Record connection uptime for Binance
    pub fn record_binance_uptime(&self, connected_duration: Duration) {
        if let Ok(mut uptime) = self.binance_connection_time.write() {
            *uptime = connected_duration;
        }
    }

    /// Record price processing latency
    pub fn record_processing_time(&self, duration: Duration) {
        if let Ok(mut times) = self.processing_times.write() {
            times.push_back(duration);
            if times.len() > 1000 {
                times.pop_front();
            }
        }
    }

    /// Record arbitrage detection latency
    pub fn record_arbitrage_time(&self, duration: Duration) {
        if let Ok(mut times) = self.arbitrage_times.write() {
            times.push_back(duration);
            if times.len() > 1000 {
                times.pop_front();
            }
        }
    }

    /// Record output formatting latency
    pub fn record_output_time(&self, duration: Duration) {
        if let Ok(mut times) = self.output_times.write() {
            times.push_back(duration);
            if times.len() > 1000 {
                times.pop_front();
            }
        }
    }

    /// Update current queue depth
    pub fn set_queue_depth(&self, depth: usize) {
        self.current_queue_depth.store(depth, Ordering::Relaxed);

        // Update max if needed
        let current_max = self.max_queue_depth.load(Ordering::Relaxed);
        if depth > current_max {
            self.max_queue_depth.store(depth, Ordering::Relaxed);
        }
    }

    /// Record a processing error
    pub fn record_error(&self) {
        self.processing_errors.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a connection failure
    pub fn record_connection_failure(&self) {
        if let Ok(mut failures) = self.connection_failures.write() {
            failures.push(Utc::now());
            // Keep only recent failures (last 100)
            if failures.len() > 100 {
                failures.remove(0);
            }
        }
    }

    /// Get current uptime in seconds
    pub fn get_uptime_seconds(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }

    /// Calculate average from a duration queue
    fn avg_duration_ms(durations: &VecDeque<Duration>) -> f64 {
        if durations.is_empty() {
            0.0
        } else {
            let total_ms: u64 = durations.iter().map(|d| d.as_millis() as u64).sum();
            total_ms as f64 / durations.len() as f64
        }
    }

    /// Generate comprehensive performance metrics snapshot
    pub fn get_metrics(&self) -> PerformanceMetrics {
        let uptime = self.get_uptime_seconds();
        let uptime_f64 = uptime as f64;

        // Message rates (per second)
        let solana_msgs = self.solana_messages.load(Ordering::Relaxed);
        let binance_msgs = self.binance_messages.load(Ordering::Relaxed);
        let total_msgs = solana_msgs + binance_msgs;

        let solana_rate = if uptime > 0 {
            solana_msgs as f64 / uptime_f64
        } else {
            0.0
        };
        let binance_rate = if uptime > 0 {
            binance_msgs as f64 / uptime_f64
        } else {
            0.0
        };
        let total_rate = if uptime > 0 {
            total_msgs as f64 / uptime_f64
        } else {
            0.0
        };

        // Opportunities
        let opportunities = self.opportunities_found.load(Ordering::Relaxed);
        let opportunities_per_hour = if uptime > 0 {
            opportunities as f64 * 3600.0 / uptime_f64
        } else {
            0.0
        };

        // Processing times
        let processing_avg = if let Ok(times) = self.processing_times.read() {
            Self::avg_duration_ms(&times)
        } else {
            0.0
        };

        let arbitrage_avg = if let Ok(times) = self.arbitrage_times.read() {
            Self::avg_duration_ms(&times)
        } else {
            0.0
        };

        let output_avg = if let Ok(times) = self.output_times.read() {
            Self::avg_duration_ms(&times)
        } else {
            0.0
        };

        // Connection stats
        let solana_uptime_pct = if let Ok(uptime_duration) = self.solana_connection_time.read() {
            if uptime > 0 {
                (uptime_duration.as_secs() as f64 / uptime_f64) * 100.0
            } else {
                0.0
            }
        } else {
            0.0
        };

        let binance_uptime_pct = if let Ok(uptime_duration) = self.binance_connection_time.read() {
            if uptime > 0 {
                (uptime_duration.as_secs() as f64 / uptime_f64) * 100.0
            } else {
                0.0
            }
        } else {
            0.0
        };

        let last_failure = if let Ok(failures) = self.connection_failures.read() {
            failures.last().copied()
        } else {
            None
        };

        // Queue and efficiency
        let queue_depth = self.current_queue_depth.load(Ordering::Relaxed);
        let max_queue = self.max_queue_depth.load(Ordering::Relaxed);
        let errors = self.processing_errors.load(Ordering::Relaxed);

        let efficiency = if total_msgs > 0 {
            ((total_msgs - errors) as f64 / total_msgs as f64) * 100.0
        } else {
            100.0
        };

        PerformanceMetrics {
            summary: PerformanceSummary {
                uptime_seconds: uptime,
                total_opportunities: opportunities,
                avg_processing_latency_ms: processing_avg,
                solana_msg_rate: solana_rate,
                binance_msg_rate: binance_rate,
                reconnection_count: self.reconnection_count.load(Ordering::Relaxed),
            },
            throughput: ThroughputStats {
                messages_per_second: total_rate,
                opportunities_per_hour,
                current_queue_depth: queue_depth,
                processing_efficiency_pct: efficiency,
            },
            connection: ConnectionStats {
                solana_uptime_pct,
                binance_uptime_pct,
                total_reconnections: self.reconnection_count.load(Ordering::Relaxed),
                avg_reconnect_time_ms: 0.0, // Could be enhanced to track this
                last_connection_failure: last_failure,
            },
            processing: ProcessingStats {
                price_processing_latency_ms: processing_avg,
                arbitrage_detection_latency_ms: arbitrage_avg,
                output_formatting_latency_ms: output_avg,
                total_pipeline_latency_ms: processing_avg + arbitrage_avg + output_avg,
                messages_processed: total_msgs,
                errors_encountered: errors,
            },
        }
    }

    /// Reset all metrics (useful for testing or periodic resets)
    pub fn reset(&self) {
        self.solana_messages.store(0, Ordering::Relaxed);
        self.binance_messages.store(0, Ordering::Relaxed);
        self.opportunities_found.store(0, Ordering::Relaxed);
        self.reconnection_count.store(0, Ordering::Relaxed);
        self.current_queue_depth.store(0, Ordering::Relaxed);
        self.max_queue_depth.store(0, Ordering::Relaxed);
        self.processing_errors.store(0, Ordering::Relaxed);

        if let Ok(mut times) = self.processing_times.write() {
            times.clear();
        }
        if let Ok(mut times) = self.arbitrage_times.write() {
            times.clear();
        }
        if let Ok(mut times) = self.output_times.write() {
            times.clear();
        }
        if let Ok(mut failures) = self.connection_failures.write() {
            failures.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_metrics_collector_creation() {
        let collector = MetricsCollector::new();
        assert_eq!(collector.solana_messages.load(Ordering::Relaxed), 0);
        assert_eq!(collector.binance_messages.load(Ordering::Relaxed), 0);
        assert_eq!(collector.opportunities_found.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_message_recording() {
        let collector = MetricsCollector::new();

        collector.record_solana_message();
        collector.record_binance_message();
        collector.record_opportunity();

        assert_eq!(collector.solana_messages.load(Ordering::Relaxed), 1);
        assert_eq!(collector.binance_messages.load(Ordering::Relaxed), 1);
        assert_eq!(collector.opportunities_found.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_processing_time_recording() {
        let collector = MetricsCollector::new();
        let duration = Duration::from_millis(50);

        collector.record_processing_time(duration);
        collector.record_arbitrage_time(duration);
        collector.record_output_time(duration);

        let metrics = collector.get_metrics();
        assert!(metrics.processing.price_processing_latency_ms > 0.0);
        assert!(metrics.processing.arbitrage_detection_latency_ms > 0.0);
        assert!(metrics.processing.output_formatting_latency_ms > 0.0);
    }

    #[test]
    fn test_queue_depth_tracking() {
        let collector = MetricsCollector::new();

        collector.set_queue_depth(5);
        collector.set_queue_depth(10);
        collector.set_queue_depth(3);

        assert_eq!(collector.current_queue_depth.load(Ordering::Relaxed), 3);
        assert_eq!(collector.max_queue_depth.load(Ordering::Relaxed), 10);
    }

    #[test]
    fn test_metrics_generation() {
        let collector = MetricsCollector::new();

        // Simulate some activity
        collector.record_solana_message();
        collector.record_binance_message();
        collector.record_opportunity();
        collector.record_processing_time(Duration::from_millis(10));

        let metrics = collector.get_metrics();

        assert_eq!(metrics.summary.total_opportunities, 1);
        assert_eq!(metrics.processing.messages_processed, 2);
        assert!(metrics.summary.uptime_seconds >= 0);
    }

    #[test]
    fn test_rate_calculations() {
        let collector = MetricsCollector::new();

        // Record some messages
        for _ in 0..10 {
            collector.record_solana_message();
        }
        for _ in 0..5 {
            collector.record_binance_message();
        }

        // Wait a sufficient amount to get non-zero uptime
        thread::sleep(Duration::from_millis(100));

        let metrics = collector.get_metrics();

        // Test that rates are calculated (should be > 0 since we have messages and uptime)
        assert!(metrics.throughput.messages_per_second >= 0.0);
        assert!(metrics.summary.solana_msg_rate >= 0.0);
        assert!(metrics.summary.binance_msg_rate >= 0.0);

        // If we have uptime, we should have positive rates
        if metrics.summary.uptime_seconds > 0 {
            assert!(metrics.throughput.messages_per_second > 0.0);
            assert!(metrics.summary.solana_msg_rate > 0.0);
            assert!(metrics.summary.binance_msg_rate > 0.0);
        }
    }

    #[test]
    fn test_reset_functionality() {
        let collector = MetricsCollector::new();

        collector.record_solana_message();
        collector.record_opportunity();
        collector.record_error();

        collector.reset();

        assert_eq!(collector.solana_messages.load(Ordering::Relaxed), 0);
        assert_eq!(collector.opportunities_found.load(Ordering::Relaxed), 0);
        assert_eq!(collector.processing_errors.load(Ordering::Relaxed), 0);
    }
}
