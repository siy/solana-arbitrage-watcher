mod config;

use clap::Parser;
use config::Config;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::parse();
    config.validate()?;

    println!("Configuration loaded successfully!");
    println!("Trading pair: {:?}", config.pair);
    println!("Profit threshold: {}%", config.threshold);
    println!("RPC providers: {:?}", config.get_rpc_providers());

    Ok(())
}
