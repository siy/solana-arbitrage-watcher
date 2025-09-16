mod config;

use clap::Parser;
use config::{Config, RawConfig};

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

    Ok(())
}
