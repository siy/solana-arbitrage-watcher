mod arbitrage;
mod config;
mod output;
mod price;
mod websocket;

use arbitrage::{calculator::FeeCalculator, detector::ArbitrageDetector};
use clap::Parser;
use config::{Config, RawConfig};
use output::{OutputFormat, OutputFormatter};
use std::sync::Arc;
use std::time::Duration;
use tokio::signal;
use websocket::ConnectionManager;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse command line arguments and validate configuration
    let raw_config = RawConfig::parse();
    let config = match Config::new(&raw_config) {
        Ok(config) => config,
        Err(errors) => {
            eprintln!("Configuration error: {}", errors);
            std::process::exit(1);
        }
    };

    // Initialize output formatter based on configuration or default to table format
    let output_format = OutputFormat::default();
    let formatter = OutputFormatter::new(output_format);

    println!("🚀 Solana Arbitrage Watcher Starting");
    println!("=====================================");
    println!("Trading pair: {:?}", config.pair);
    println!("Profit threshold: {}%", config.threshold.value());
    println!("Max price age: {}ms", config.max_price_age_ms.value());
    println!("Output format: {}", output_format);
    println!("RPC providers: {:?}", config.rpc_providers);
    println!();

    println!("🔗 Starting WebSocket connections...");

    // Create WebSocket connection manager
    let connection_manager = ConnectionManager::new(&config)?;

    // Start WebSocket connections and get the price cache
    let price_cache = connection_manager.start().await?;

    // Create fee calculator with default settings
    let fee_calculator = FeeCalculator::default();

    // Create arbitrage detector
    let arbitrage_detector = ArbitrageDetector::new(
        Arc::clone(&price_cache),
        &config,
        fee_calculator,
    );

    println!("✅ Price data available, starting arbitrage detection");
    println!();

    // Main arbitrage detection loop
    let detection_handle = {
        let mut detector = arbitrage_detector;
        let formatter = formatter;
        let trading_pair = config.pair;

        tokio::spawn(async move {
            let mut detection_interval = tokio::time::interval(Duration::from_secs(1));

            loop {
                detection_interval.tick().await;

                match detector.check_for_opportunities().await {
                    Ok(Some(opportunity)) => {
                        println!("{}", formatter.format_opportunity(&opportunity));
                        println!();
                    }
                    Ok(None) => {
                        // Only show "no opportunities" message periodically to avoid spam
                        if detector.stats().total_checks % 60 == 0 {
                            println!("{}", formatter.format_no_opportunities(trading_pair));
                            println!();
                        }
                    }
                    Err(e) => {
                        println!("{}", formatter.format_error(&e.to_string()));
                        println!();
                    }
                }
            }
        })
    };

    // Wait for shutdown signal (Ctrl+C)
    println!("🔍 Monitoring for arbitrage opportunities... (Press Ctrl+C to stop)");
    signal::ctrl_c().await?;

    println!("\n🛑 Shutdown signal received, stopping...");

    // Cancel the detection task
    detection_handle.abort();

    // Wait a moment for graceful shutdown
    tokio::time::sleep(Duration::from_millis(500)).await;

    println!("✅ Arbitrage watcher stopped");

    Ok(())
}
