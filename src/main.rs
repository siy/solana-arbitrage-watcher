mod arbitrage;
mod config;
mod output;
mod performance;
mod price;
#[cfg(test)]
mod test_utils;
mod util;
mod websocket;

use arbitrage::{calculator::FeeCalculator, detector::ArbitrageDetector};
use clap::Parser;
use config::{Config, RawConfig};
use log::{error, info};
use output::OutputFormatter;
use performance::{MonitorConfig, PerformanceMonitor};
use std::sync::Arc;
use std::time::Duration;
use tokio::signal;
use websocket::ConnectionManager;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();

    // Parse command line arguments and validate configuration
    let raw_config = RawConfig::parse();
    let config = match Config::new(&raw_config) {
        Ok(config) => config,
        Err(errors) => {
            error!("Configuration error: {}", errors);
            std::process::exit(1);
        }
    };

    // Initialize output formatter from configuration
    let formatter = OutputFormatter::new(config.output_format);

    info!("Solana Arbitrage Watcher Starting");
    info!("Trading pair: {:?}", config.pair);
    info!("Profit threshold: {}%", config.threshold.value());
    info!("Max price age: {}ms", config.max_price_age_ms.value());
    info!("Output format: {}", config.output_format);
    // Avoid logging full URLs (may contain credentials/keys)
    info!("RPC providers configured: {}", config.rpc_providers.len());
    if config.api_keys.has_keys() {
        info!("Authenticated RPC access enabled");
    } else {
        info!("Using public RPC endpoints");
    }

    // Initialize performance monitoring
    let monitor_config = MonitorConfig {
        reporting_interval: Duration::from_secs(60),
        enabled: true,
        detailed_logging: false,
    };
    let performance_monitor = PerformanceMonitor::new(monitor_config);
    let metrics = performance_monitor.metrics();

    info!("Starting performance monitoring...");
    performance_monitor.start_monitoring().await;

    info!("Starting WebSocket connections...");

    // Create WebSocket connection manager with metrics
    let connection_manager = ConnectionManager::new(&config)?.with_metrics(Arc::clone(&metrics));

    // Start WebSocket connections and get the price cache with shutdown handles
    let (price_cache, binance_handle, solana_handle) = connection_manager.start_with_handles();

    // Create fee calculator with default settings
    let fee_calculator = FeeCalculator::default();

    // Create arbitrage detector with metrics
    let arbitrage_detector =
        ArbitrageDetector::new(Arc::clone(&price_cache), &config, fee_calculator)
            .with_metrics(Arc::clone(&metrics));

    info!("Price data available, starting arbitrage detection");
    println!();

    // Main arbitrage detection loop
    let detection_handle = {
        let mut detector = arbitrage_detector;
        let trading_pair = config.pair;
        let metrics_clone = Arc::clone(&metrics);

        tokio::spawn(async move {
            let mut detection_interval = tokio::time::interval(Duration::from_secs(1));

            loop {
                detection_interval.tick().await;

                // Record arbitrage detection timing
                let detection_start = std::time::Instant::now();
                let result = detector.check_for_opportunities().await;
                let detection_duration = detection_start.elapsed();
                metrics_clone.record_arbitrage_time(detection_duration);

                // Update queue depth (simplified - could be enhanced to track actual queue)
                metrics_clone.set_queue_depth(0);

                match result {
                    Ok(Some(opportunity)) => {
                        metrics_clone.record_opportunity();

                        // Record output formatting timing
                        let output_start = std::time::Instant::now();
                        let formatted_output = formatter.format_opportunity(&opportunity);
                        let output_duration = output_start.elapsed();
                        metrics_clone.record_output_time(output_duration);

                        println!("{}", formatted_output);
                        println!();
                    }
                    Ok(None) => {
                        // Only show "no opportunities" message periodically to avoid spam
                        if detector.stats().total_checks % 60 == 0 {
                            let output_start = std::time::Instant::now();
                            let formatted_output = formatter.format_no_opportunities(trading_pair);
                            let output_duration = output_start.elapsed();
                            metrics_clone.record_output_time(output_duration);

                            println!("{}", formatted_output);
                            println!();
                        }
                    }
                    Err(e) => {
                        metrics_clone.record_error();

                        let output_start = std::time::Instant::now();
                        let formatted_output = formatter.format_error(&e.to_string());
                        let output_duration = output_start.elapsed();
                        metrics_clone.record_output_time(output_duration);

                        println!("{}", formatted_output);
                        println!();
                    }
                }
            }
        })
    };

    // Wait for shutdown signal (Ctrl+C)
    info!("Monitoring for arbitrage opportunities... (Press Ctrl+C to stop)");
    signal::ctrl_c().await?;

    info!("Shutdown signal received, stopping...");

    // Cancel all tasks
    detection_handle.abort();
    binance_handle.abort();
    solana_handle.abort();

    // Wait a moment for graceful shutdown
    tokio::time::sleep(Duration::from_millis(500)).await;

    info!("Arbitrage watcher stopped");

    Ok(())
}
