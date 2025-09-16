use crate::config::TradingPair;
use crate::price::PriceSource;

/// Format a trading pair as a human-readable string
pub fn format_trading_pair(pair: TradingPair) -> &'static str {
    match pair {
        TradingPair::SolUsdt => "SOL/USDT",
        TradingPair::SolUsdc => "SOL/USDC",
    }
}

/// Format a price source as a human-readable string
pub fn format_price_source(source: PriceSource) -> &'static str {
    match source {
        PriceSource::Solana => "Solana",
        PriceSource::Binance => "Binance",
    }
}

/// Round a value to specified precision
pub fn round_to_precision(value: f64, precision: usize) -> f64 {
    let multiplier = 10_f64.powi(precision as i32);
    (value * multiplier).round() / multiplier
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_trading_pair() {
        assert_eq!(format_trading_pair(TradingPair::SolUsdt), "SOL/USDT");
        assert_eq!(format_trading_pair(TradingPair::SolUsdc), "SOL/USDC");
    }

    #[test]
    fn test_format_price_source() {
        assert_eq!(format_price_source(PriceSource::Solana), "Solana");
        assert_eq!(format_price_source(PriceSource::Binance), "Binance");
    }

    #[test]
    fn test_round_to_precision() {
        assert_eq!(round_to_precision(195.123456, 2), 195.12);
        assert_eq!(round_to_precision(195.126, 2), 195.13);
        assert_eq!(round_to_precision(195.0, 4), 195.0);
    }
}
