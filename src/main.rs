mod config;
mod price;
mod websocket;

use clap::Parser;
use config::{Config, RawConfig};
use price::{PriceCache, PriceSource, PriceUpdate};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let raw_config = RawConfig::parse();
    let config = match Config::new(&raw_config) {
        Ok(config) => config,
        Err(errors) => {
            eprintln!("{}", errors);
            std::process::exit(1);
        }
    };

    println!("Configuration loaded successfully!");
    println!("Trading pair: {:?}", config.pair);
    println!("Profit threshold: {}%", config.threshold.value());
    println!("Max price age: {}ms", config.max_price_age_ms.value());
    println!("RPC providers: {:?}", config.rpc_providers);

    // Demonstrate price types (for development only)
    if cfg!(debug_assertions) {
        println!("\nDemonstrating price types:");
        let price_cache = PriceCache::new();

        // Simulate price updates
        let solana_update = PriceUpdate::new(PriceSource::Solana, config.pair, 195.50);
        let binance_update = PriceUpdate::new(PriceSource::Binance, config.pair, 195.75);

        println!(
            "Solana update: {} at ${:.2}",
            PriceSource::Solana.display_name(),
            solana_update.price
        );
        println!(
            "Binance update: {} at ${:.2}",
            PriceSource::Binance.display_name(),
            binance_update.price
        );

        // Update cache
        price_cache.update(&solana_update);
        price_cache.update(&binance_update);

        // Check fresh prices
        if price_cache.has_fresh_prices(config.max_price_age_ms.value()) {
            if let Some((solana_price, binance_price)) = price_cache.get_both_prices() {
                println!(
                    "Fresh prices available: {} ${:.2}, {} ${:.2}",
                    solana_price.source.display_name(),
                    solana_price.price,
                    binance_price.source.display_name(),
                    binance_price.price
                );
            }
        }
    }

    Ok(())
}
