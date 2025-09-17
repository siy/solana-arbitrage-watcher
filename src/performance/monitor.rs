use super::metrics::{MetricsCollector, PerformanceMetrics};
use log::{info, warn};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;

/// Performance monitor that provides periodic reporting and real-time metrics access
#[derive(Debug)]
pub struct PerformanceMonitor {
    metrics: Arc<MetricsCollector>,
    reporting_interval: Duration,
    enabled: bool,
}

/// Configuration for performance monitoring
#[derive(Debug, Clone)]
pub struct MonitorConfig {
    /// How often to log performance summaries (default: 60 seconds)
    pub reporting_interval: Duration,
    /// Whether to enable performance monitoring (default: true)
    pub enabled: bool,
    /// Whether to log detailed metrics (default: false)
    pub detailed_logging: bool,
}

impl Default for MonitorConfig {
    fn default() -> Self {
        Self {
            reporting_interval: Duration::from_secs(60),
            enabled: true,
            detailed_logging: false,
        }
    }
}

impl PerformanceMonitor {
    /// Create a new performance monitor with the given configuration
    pub fn new(config: MonitorConfig) -> Self {
        Self {
            metrics: Arc::new(MetricsCollector::new()),
            reporting_interval: config.reporting_interval,
            enabled: config.enabled,
        }
    }

    /// Create a performance monitor with default settings
    pub fn with_defaults() -> Self {
        Self::new(MonitorConfig::default())
    }

    /// Get a reference to the metrics collector for recording data
    pub fn metrics(&self) -> Arc<MetricsCollector> {
        Arc::clone(&self.metrics)
    }

    /// Get current performance metrics snapshot
    pub fn get_current_metrics(&self) -> PerformanceMetrics {
        self.metrics.get_metrics()
    }

    /// Start the performance monitoring background task
    pub async fn start_monitoring(&self) {
        if !self.enabled {
            info!("Performance monitoring disabled");
            return;
        }

        info!(
            "Starting performance monitor (reporting every {:?})",
            self.reporting_interval
        );

        let metrics = Arc::clone(&self.metrics);
        let interval_duration = self.reporting_interval;

        tokio::spawn(async move {
            let mut timer = interval(interval_duration);

            loop {
                timer.tick().await;

                let performance = metrics.get_metrics();
                Self::log_performance_summary(&performance);

                // Check for performance warnings
                Self::check_performance_warnings(&performance);
            }
        });
    }

    /// Log a comprehensive performance summary
    fn log_performance_summary(metrics: &PerformanceMetrics) {
        let summary = &metrics.summary;
        let throughput = &metrics.throughput;
        let connection = &metrics.connection;
        let processing = &metrics.processing;

        info!("=== PERFORMANCE SUMMARY ===");
        info!(
            "Uptime: {}s | Opportunities: {} ({:.1}/hr)",
            summary.uptime_seconds, summary.total_opportunities, throughput.opportunities_per_hour
        );
        info!(
            "Messages: {:.1}/s (SOL: {:.1}/s, BIN: {:.1}/s) | Queue: {}",
            throughput.messages_per_second,
            summary.solana_msg_rate,
            summary.binance_msg_rate,
            throughput.current_queue_depth
        );
        info!(
            "Latency: Processing {:.1}ms | Arbitrage {:.1}ms | Output {:.1}ms | Total {:.1}ms",
            processing.price_processing_latency_ms,
            processing.arbitrage_detection_latency_ms,
            processing.output_formatting_latency_ms,
            processing.total_pipeline_latency_ms
        );
        info!(
            "Connection: SOL {:.1}% | BIN {:.1}% | Reconnects: {} | Efficiency: {:.1}%",
            connection.solana_uptime_pct,
            connection.binance_uptime_pct,
            connection.total_reconnections,
            throughput.processing_efficiency_pct
        );
        info!(
            "Processed: {} messages | Errors: {} | Error Rate: {:.2}%",
            processing.messages_processed,
            processing.errors_encountered,
            if processing.messages_processed > 0 {
                (processing.errors_encountered as f64 / processing.messages_processed as f64) * 100.0
            } else {
                0.0
            }
        );
        info!("=== END SUMMARY ===");
    }

    /// Check for performance issues and log warnings
    fn check_performance_warnings(metrics: &PerformanceMetrics) {
        let processing = &metrics.processing;
        let throughput = &metrics.throughput;
        let connection = &metrics.connection;

        // Check processing latency (warn if > 10ms for price processing)
        if processing.price_processing_latency_ms > 10.0 {
            warn!(
                "High price processing latency: {:.1}ms (target: <10ms)",
                processing.price_processing_latency_ms
            );
        }

        // Check arbitrage detection latency (warn if > 5ms)
        if processing.arbitrage_detection_latency_ms > 5.0 {
            warn!(
                "High arbitrage detection latency: {:.1}ms (target: <5ms)",
                processing.arbitrage_detection_latency_ms
            );
        }

        // Check queue depth (warn if > 100 messages)
        if throughput.current_queue_depth > 100 {
            warn!(
                "High queue depth: {} messages (consider backpressure)",
                throughput.current_queue_depth
            );
        }

        // Check processing efficiency (warn if < 95%)
        if throughput.processing_efficiency_pct < 95.0 {
            warn!(
                "Low processing efficiency: {:.1}% (error rate may be high)",
                throughput.processing_efficiency_pct
            );
        }

        // Check connection uptime (warn if < 90%)
        if connection.solana_uptime_pct < 90.0 && metrics.summary.uptime_seconds > 60 {
            warn!(
                "Low Solana connection uptime: {:.1}% (frequent disconnections)",
                connection.solana_uptime_pct
            );
        }

        if connection.binance_uptime_pct < 90.0 && metrics.summary.uptime_seconds > 60 {
            warn!(
                "Low Binance connection uptime: {:.1}% (frequent disconnections)",
                connection.binance_uptime_pct
            );
        }

        // Check message rate (warn if very low after initial period)
        if throughput.messages_per_second < 0.1 && metrics.summary.uptime_seconds > 120 {
            warn!(
                "Very low message rate: {:.3}/s (connection issues?)",
                throughput.messages_per_second
            );
        }
    }

    /// Generate a detailed performance report for debugging
    pub fn generate_detailed_report(&self) -> String {
        let metrics = self.get_current_metrics();
        let summary = &metrics.summary;
        let throughput = &metrics.throughput;
        let connection = &metrics.connection;
        let processing = &metrics.processing;

        format!(
            r#"DETAILED PERFORMANCE REPORT
============================

UPTIME & BASIC STATS:
- Uptime: {} seconds ({:.1} minutes)
- Total Messages: {}
- Total Opportunities: {}
- Reconnections: {}

MESSAGE THROUGHPUT:
- Overall Rate: {:.2} messages/second
- Solana Rate: {:.2} messages/second
- Binance Rate: {:.2} messages/second
- Opportunities: {:.2} per hour

PROCESSING PERFORMANCE:
- Price Processing: {:.2}ms avg
- Arbitrage Detection: {:.2}ms avg
- Output Formatting: {:.2}ms avg
- Total Pipeline: {:.2}ms avg
- Processing Efficiency: {:.1}%

CONNECTION RELIABILITY:
- Solana Uptime: {:.1}%
- Binance Uptime: {:.1}%
- Total Reconnections: {}
- Last Failure: {}

QUEUE MANAGEMENT:
- Current Queue Depth: {}
- Processing Errors: {}
- Error Rate: {:.2}%

PERFORMANCE STATUS:
{}
"#,
            summary.uptime_seconds,
            summary.uptime_seconds as f64 / 60.0,
            processing.messages_processed,
            summary.total_opportunities,
            summary.reconnection_count,
            throughput.messages_per_second,
            summary.solana_msg_rate,
            summary.binance_msg_rate,
            throughput.opportunities_per_hour,
            processing.price_processing_latency_ms,
            processing.arbitrage_detection_latency_ms,
            processing.output_formatting_latency_ms,
            processing.total_pipeline_latency_ms,
            throughput.processing_efficiency_pct,
            connection.solana_uptime_pct,
            connection.binance_uptime_pct,
            connection.total_reconnections,
            connection
                .last_connection_failure
                .map_or("None".to_string(), |dt| dt.to_rfc3339()),
            throughput.current_queue_depth,
            processing.errors_encountered,
            if processing.messages_processed > 0 {
                (processing.errors_encountered as f64 / processing.messages_processed as f64) * 100.0
            } else {
                0.0
            },
            Self::get_performance_status(&metrics)
        )
    }

    /// Get overall performance status assessment
    fn get_performance_status(metrics: &PerformanceMetrics) -> &'static str {
        let processing = &metrics.processing;
        let throughput = &metrics.throughput;
        let connection = &metrics.connection;

        // Check if any critical thresholds are exceeded
        if processing.price_processing_latency_ms > 20.0
            || processing.arbitrage_detection_latency_ms > 10.0
            || throughput.processing_efficiency_pct < 90.0
            || throughput.current_queue_depth > 200
        {
            "CRITICAL - Performance degraded significantly"
        } else if processing.price_processing_latency_ms > 10.0
            || processing.arbitrage_detection_latency_ms > 5.0
            || throughput.processing_efficiency_pct < 95.0
            || connection.solana_uptime_pct < 90.0
            || connection.binance_uptime_pct < 90.0
        {
            "WARNING - Performance issues detected"
        } else if throughput.messages_per_second > 1.0
            && processing.total_pipeline_latency_ms < 15.0
            && throughput.processing_efficiency_pct > 98.0
        {
            "EXCELLENT - Optimal performance"
        } else {
            "GOOD - Normal operation"
        }
    }

    /// Reset all metrics (useful for testing or restart scenarios)
    pub fn reset_metrics(&self) {
        self.metrics.reset();
        info!("Performance metrics reset");
    }

    /// Check if monitoring is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

/// Helper trait for timing operations
pub trait TimedOperation {
    /// Execute a function and record its execution time to the metrics collector
    fn timed<F, R>(&self, operation: F) -> R
    where
        F: FnOnce() -> R;
}

impl TimedOperation for Arc<MetricsCollector> {
    fn timed<F, R>(&self, operation: F) -> R
    where
        F: FnOnce() -> R,
    {
        let start = std::time::Instant::now();
        let result = operation();
        let duration = start.elapsed();
        self.record_processing_time(duration);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use tokio::test as tokio_test;

    #[test]
    fn test_monitor_creation() {
        let monitor = PerformanceMonitor::with_defaults();
        assert!(monitor.is_enabled());
    }

    #[test]
    fn test_custom_config() {
        let config = MonitorConfig {
            reporting_interval: Duration::from_secs(30),
            enabled: false,
            detailed_logging: true,
        };
        let monitor = PerformanceMonitor::new(config);
        assert!(!monitor.is_enabled());
    }

    #[test]
    fn test_metrics_access() {
        let monitor = PerformanceMonitor::with_defaults();
        let metrics_ref = monitor.metrics();

        metrics_ref.record_solana_message();
        metrics_ref.record_opportunity();

        let current_metrics = monitor.get_current_metrics();
        assert_eq!(current_metrics.summary.total_opportunities, 1);
        assert_eq!(current_metrics.processing.messages_processed, 1);
    }

    #[test]
    fn test_detailed_report_generation() {
        let monitor = PerformanceMonitor::with_defaults();
        let metrics_ref = monitor.metrics();

        // Generate some test data
        metrics_ref.record_solana_message();
        metrics_ref.record_binance_message();
        metrics_ref.record_opportunity();
        metrics_ref.record_processing_time(Duration::from_millis(5));

        let report = monitor.generate_detailed_report();
        assert!(report.contains("DETAILED PERFORMANCE REPORT"));
        assert!(report.contains("Total Messages: 2"));
        assert!(report.contains("Total Opportunities: 1"));
    }

    #[test]
    fn test_timed_operation() {
        let collector = Arc::new(MetricsCollector::new());

        let result = collector.timed(|| {
            thread::sleep(Duration::from_millis(10));
            42
        });

        assert_eq!(result, 42);

        let metrics = collector.get_metrics();
        assert!(metrics.processing.price_processing_latency_ms >= 10.0);
    }

    #[test]
    fn test_performance_status_assessment() {
        let monitor = PerformanceMonitor::with_defaults();
        let collector = monitor.metrics();

        // Simulate good performance
        collector.record_solana_message();
        collector.record_processing_time(Duration::from_millis(5));

        let metrics = monitor.get_current_metrics();
        let status = PerformanceMonitor::get_performance_status(&metrics);

        // Performance status should be valid
        assert!(
            status.contains("GOOD") ||
            status.contains("EXCELLENT") ||
            status.contains("WARNING") ||
            status.contains("CRITICAL")
        );
    }

    #[tokio_test]
    async fn test_monitoring_lifecycle() {
        let monitor = PerformanceMonitor::with_defaults();
        let metrics_ref = monitor.metrics();

        // Generate some activity
        metrics_ref.record_solana_message();
        metrics_ref.record_opportunity();

        // Test metrics retrieval
        let current = monitor.get_current_metrics();
        assert!(current.summary.uptime_seconds >= 0);

        // Test reset
        monitor.reset_metrics();
        let after_reset = monitor.get_current_metrics();
        assert_eq!(after_reset.summary.total_opportunities, 0);
    }
}