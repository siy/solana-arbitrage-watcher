use crate::arbitrage::calculator::ArbitrageOpportunity;
use crate::config::TradingPair;
use crate::price::ValidatedPricePair;
use crate::util::{format_price_source, format_trading_pair, round_to_precision};
use serde_json::json;
use std::fmt;

/// Output format options for displaying arbitrage data
#[derive(Debug, Clone, Copy, clap::ValueEnum, Default)]
pub enum OutputFormat {
    /// Human-readable table format
    #[default]
    Table,
    /// JSON format for machine processing
    Json,
    /// Compact single-line format
    Compact,
}

/// Formatter for displaying arbitrage opportunities and price data
pub struct OutputFormatter {
    format: OutputFormat,
    show_timestamps: bool,
    precision: usize,
}

impl OutputFormatter {
    /// Create new formatter with specified format
    pub fn new(format: OutputFormat) -> Self {
        Self {
            format,
            show_timestamps: true,
            precision: 4,
        }
    }

    /// Create formatter with custom settings
    #[allow(dead_code)] // Future feature: configurable settings
    pub fn with_settings(format: OutputFormat, show_timestamps: bool, precision: usize) -> Self {
        Self {
            format,
            show_timestamps,
            precision,
        }
    }

    /// Format an arbitrage opportunity for display
    pub fn format_opportunity(&self, opportunity: &ArbitrageOpportunity) -> String {
        match self.format {
            OutputFormat::Table => self.format_opportunity_table(opportunity),
            OutputFormat::Json => self.format_opportunity_json(opportunity),
            OutputFormat::Compact => self.format_opportunity_compact(opportunity),
        }
    }

    /// Format price pair information
    #[allow(dead_code)] // Future feature: real-time price display
    pub fn format_price_pair(&self, prices: &ValidatedPricePair, pair: TradingPair) -> String {
        match self.format {
            OutputFormat::Table => self.format_price_pair_table(prices, pair),
            OutputFormat::Json => self.format_price_pair_json(prices, pair),
            OutputFormat::Compact => self.format_price_pair_compact(prices, pair),
        }
    }

    /// Format arbitrage opportunity as a table
    fn format_opportunity_table(&self, opportunity: &ArbitrageOpportunity) -> String {
        let mut output = String::new();

        output.push_str("ARBITRAGE OPPORTUNITY DETECTED\n");
        output.push_str("=".repeat(50).as_str());
        output.push('\n');

        output.push_str(&format!(
            "Buy Source:       {} @ ${:.prec$}\n",
            format_price_source(opportunity.buy_source),
            opportunity.buy_price,
            prec = self.precision
        ));

        output.push_str(&format!(
            "Sell Source:      {} @ ${:.prec$}\n",
            format_price_source(opportunity.sell_source),
            opportunity.sell_price,
            prec = self.precision
        ));

        output.push_str(&format!(
            "Raw Profit:       ${:.prec$} per unit\n",
            opportunity.raw_profit_per_unit,
            prec = self.precision
        ));

        output.push_str(&format!(
            "Net Profit:       ${:.prec$} per unit\n",
            opportunity.net_profit_per_unit,
            prec = self.precision
        ));

        output.push_str(&format!(
            "Profit Margin:    {:.2}%\n",
            opportunity.profit_percentage
        ));

        output.push_str(&format!(
            "Total Fees:      ${:.prec$} per unit\n",
            opportunity.total_fees_per_unit,
            prec = self.precision
        ));

        output.push_str(&format!(
            "Recommended Amount: {:.prec$} {}\n",
            opportunity.recommended_amount,
            format_trading_pair(opportunity.trading_pair)
                .split('/')
                .next()
                .unwrap_or("SOL"),
            prec = self.precision
        ));

        output.push_str(&format!(
            "Est. Total Profit: ${:.prec$}\n",
            opportunity.estimated_total_profit,
            prec = self.precision
        ));

        if self.show_timestamps {
            output.push_str(&format!(
                "Detected at:      {}\n",
                chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
            ));
        }

        output.push_str("=".repeat(50).as_str());
        output
    }

    /// Format arbitrage opportunity as JSON
    fn format_opportunity_json(&self, opportunity: &ArbitrageOpportunity) -> String {
        let mut json_obj = json!({
            "type": "arbitrage_opportunity",
            "trading_pair": format_trading_pair(opportunity.trading_pair).to_lowercase(),
            "buy_source": format_price_source(opportunity.buy_source).to_lowercase(),
            "sell_source": format_price_source(opportunity.sell_source).to_lowercase(),
            "buy_price": round_to_precision(opportunity.buy_price, self.precision),
            "sell_price": round_to_precision(opportunity.sell_price, self.precision),
            "raw_profit_per_unit": round_to_precision(opportunity.raw_profit_per_unit, self.precision),
            "net_profit_per_unit": round_to_precision(opportunity.net_profit_per_unit, self.precision),
            "profit_percentage": round_to_precision(opportunity.profit_percentage, 2),
            "total_fees_per_unit": round_to_precision(opportunity.total_fees_per_unit, self.precision),
            "recommended_amount": round_to_precision(opportunity.recommended_amount, self.precision),
            "estimated_total_profit": round_to_precision(opportunity.estimated_total_profit, self.precision),
        });

        if self.show_timestamps {
            if let serde_json::Value::Object(ref mut map) = json_obj {
                map.insert(
                    "timestamp".to_string(),
                    json!(chrono::Utc::now().to_rfc3339()),
                );
            }
        }

        serde_json::to_string_pretty(&json_obj).unwrap_or_else(|_| "{}".to_string())
    }

    /// Format arbitrage opportunity in compact format
    fn format_opportunity_compact(&self, opportunity: &ArbitrageOpportunity) -> String {
        format!(
            "ARBITRAGE {}: Buy {} @ ${:.prec$} -> Sell {} @ ${:.prec$} | Profit: {:.2}% (${:.prec$} total)",
            format_trading_pair(opportunity.trading_pair),
            format_price_source(opportunity.buy_source),
            opportunity.buy_price,
            format_price_source(opportunity.sell_source),
            opportunity.sell_price,
            opportunity.profit_percentage,
            opportunity.estimated_total_profit,
            prec = self.precision
        )
    }

    /// Format price pair as table
    fn format_price_pair_table(&self, prices: &ValidatedPricePair, pair: TradingPair) -> String {
        let mut output = String::new();

        output.push_str(&format!("{} PRICE UPDATE\n", format_trading_pair(pair)));
        output.push_str("-".repeat(30).as_str());
        output.push('\n');

        output.push_str(&format!(
            "Solana:    ${:.prec$} (age: {}ms)\n",
            prices.solana_price.price,
            prices.solana_price.age_ms(),
            prec = self.precision
        ));

        output.push_str(&format!(
            "Binance:   ${:.prec$} (age: {}ms)\n",
            prices.binance_price.price,
            prices.binance_price.age_ms(),
            prec = self.precision
        ));

        output.push_str(&format!(
            "Spread:    ${:.prec$} ({:.2}%)\n",
            prices.price_spread,
            prices.price_spread_percentage,
            prec = self.precision
        ));

        if self.show_timestamps {
            output.push_str(&format!(
                "Updated:   {}\n",
                chrono::Utc::now().format("%H:%M:%S")
            ));
        }

        output.push_str("-".repeat(30).as_str());
        output
    }

    /// Format price pair as JSON
    fn format_price_pair_json(&self, prices: &ValidatedPricePair, pair: TradingPair) -> String {
        let mut json_obj = json!({
            "type": "price_update",
            "trading_pair": format_trading_pair(pair).to_lowercase(),
            "solana_price": round_to_precision(prices.solana_price.price, self.precision),
            "binance_price": round_to_precision(prices.binance_price.price, self.precision),
            "price_spread": round_to_precision(prices.price_spread, self.precision),
            "spread_percentage": round_to_precision(prices.price_spread_percentage, 2),
            "solana_age_ms": prices.solana_price.age_ms(),
            "binance_age_ms": prices.binance_price.age_ms(),
        });

        if self.show_timestamps {
            if let serde_json::Value::Object(ref mut map) = json_obj {
                map.insert(
                    "timestamp".to_string(),
                    json!(chrono::Utc::now().to_rfc3339()),
                );
            }
        }

        serde_json::to_string_pretty(&json_obj).unwrap_or_else(|_| "{}".to_string())
    }

    /// Format price pair in compact format
    fn format_price_pair_compact(&self, prices: &ValidatedPricePair, pair: TradingPair) -> String {
        format!(
            "{}: SOL ${:.prec$} | BIN ${:.prec$} | Spread: {:.2}%",
            format_trading_pair(pair),
            prices.solana_price.price,
            prices.binance_price.price,
            prices.price_spread_percentage,
            prec = self.precision
        )
    }

    /// Format no opportunities message
    pub fn format_no_opportunities(&self, pair: TradingPair) -> String {
        match self.format {
            OutputFormat::Table => format!(
                "No arbitrage opportunities found for {}\n{}",
                format_trading_pair(pair),
                "-".repeat(40)
            ),
            OutputFormat::Json => json!({
                "type": "no_opportunities",
                "trading_pair": format_trading_pair(pair).to_lowercase(),
                "timestamp": chrono::Utc::now().to_rfc3339()
            })
            .to_string(),
            OutputFormat::Compact => format!("No opportunities: {}", format_trading_pair(pair)),
        }
    }

    /// Format error message
    pub fn format_error(&self, error: &str) -> String {
        match self.format {
            OutputFormat::Table => format!("ERROR: {}\n{}", error, "!".repeat(error.len() + 7)),
            OutputFormat::Json => {
                let json_obj = json!({
                    "type": "error",
                    "message": error,
                    "timestamp": chrono::Utc::now().to_rfc3339()
                });
                serde_json::to_string_pretty(&json_obj).unwrap_or_else(|_| "{}".to_string())
            }
            OutputFormat::Compact => format!("ERROR: {}", error),
        }
    }
}

impl Default for OutputFormatter {
    fn default() -> Self {
        Self::new(OutputFormat::Table)
    }
}

impl fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OutputFormat::Table => write!(f, "table"),
            OutputFormat::Json => write!(f, "json"),
            OutputFormat::Compact => write!(f, "compact"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arbitrage::calculator::ArbitrageOpportunity;
    use crate::price::{PriceSource, SourcePrice, ValidatedPricePair};

    fn create_test_opportunity() -> ArbitrageOpportunity {
        ArbitrageOpportunity {
            buy_source: PriceSource::Binance,
            sell_source: PriceSource::Solana,
            buy_price: 195.0,
            sell_price: 196.0,
            raw_profit_per_unit: 1.0,
            net_profit_per_unit: 0.75,
            profit_percentage: 0.38,
            total_fees_per_unit: 0.25,
            trading_pair: TradingPair::SolUsdt,
            recommended_amount: 10.0,
            estimated_total_profit: 7.5,
        }
    }

    fn create_test_price_pair() -> ValidatedPricePair {
        let solana_price = SourcePrice::new(196.0, PriceSource::Solana);
        let binance_price = SourcePrice::new(195.0, PriceSource::Binance);
        ValidatedPricePair::new(solana_price, binance_price)
    }

    #[test]
    fn test_table_format_opportunity() {
        let formatter = OutputFormatter::new(OutputFormat::Table);
        let opportunity = create_test_opportunity();
        let output = formatter.format_opportunity(&opportunity);

        assert!(output.contains("ARBITRAGE OPPORTUNITY"));
        assert!(output.contains("Binance @ $195.0000"));
        assert!(output.contains("Solana @ $196.0000"));
        assert!(output.contains("0.38%"));
    }

    #[test]
    fn test_json_format_opportunity() {
        let formatter = OutputFormatter::new(OutputFormat::Json);
        let opportunity = create_test_opportunity();
        let output = formatter.format_opportunity(&opportunity);

        assert!(output.contains("\"type\": \"arbitrage_opportunity\""));
        assert!(output.contains("\"buy_source\": \"binance\""));
        assert!(output.contains("\"sell_source\": \"solana\""));
    }

    #[test]
    fn test_compact_format_opportunity() {
        let formatter = OutputFormatter::new(OutputFormat::Compact);
        let opportunity = create_test_opportunity();
        let output = formatter.format_opportunity(&opportunity);

        assert!(output.contains("ARBITRAGE SOL/USDT: Buy Binance"));
        assert!(output.contains("Sell Solana"));
        assert!(output.contains("0.38%"));
    }

    #[test]
    fn test_price_pair_formatting() {
        let formatter = OutputFormatter::new(OutputFormat::Table);
        let prices = create_test_price_pair();
        let output = formatter.format_price_pair(&prices, TradingPair::SolUsdt);

        assert!(output.contains("SOL/USDT PRICE UPDATE"));
        assert!(output.contains("Solana:    $196.0000"));
        assert!(output.contains("Binance:   $195.0000"));
    }

    #[test]
    fn test_precision_setting() {
        let formatter = OutputFormatter::with_settings(OutputFormat::Table, false, 2);
        let opportunity = create_test_opportunity();
        let output = formatter.format_opportunity(&opportunity);

        assert!(output.contains("$195.00"));
        assert!(output.contains("$196.00"));
    }

    #[test]
    fn test_no_opportunities_format() {
        let formatter = OutputFormatter::new(OutputFormat::Table);
        let output = formatter.format_no_opportunities(TradingPair::SolUsdt);

        assert!(output.contains("No arbitrage opportunities"));
        assert!(output.contains("SOL/USDT"));
    }

    #[test]
    fn test_error_format() {
        let formatter = OutputFormatter::new(OutputFormat::Json);
        let output = formatter.format_error("Connection failed");

        println!("Actual output: {}", output);
        assert!(output.contains("\"type\": \"error\""));
        assert!(output.contains("\"message\": \"Connection failed\""));
    }

    #[test]
    fn test_output_format_display() {
        assert_eq!(OutputFormat::Table.to_string(), "table");
        assert_eq!(OutputFormat::Json.to_string(), "json");
        assert_eq!(OutputFormat::Compact.to_string(), "compact");
    }
}
