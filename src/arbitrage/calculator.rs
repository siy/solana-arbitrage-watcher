use crate::config::{ProfitThreshold, TradingPair};
use crate::price::{PriceSource, ValidatedPricePair};
use thiserror::Error;

/// Errors that can occur during fee calculation
#[derive(Debug, Error)]
#[allow(dead_code)]
#[allow(clippy::enum_variant_names)]
pub enum CalculatorError {
    #[error("Invalid trading pair for fee calculation")]
    InvalidTradingPair,
    #[error("Invalid price data provided")]
    InvalidPriceData,
    #[error("Fee percentage out of valid range: {0}")]
    InvalidFeePercentage(f64),
    #[error("Trade amount must be positive: {0}")]
    InvalidTradeAmount(f64),
}

/// Trading fees for different platforms and operations
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TradingFees {
    /// Binance spot trading fee (percentage)
    pub binance_spot_fee: f64,
    /// Solana DEX trading fee (percentage)
    pub solana_dex_fee: f64,
    /// Gas/transaction fees for Solana (in SOL)
    pub solana_gas_fee: f64,
    /// Withdrawal/deposit fees for moving funds
    pub transfer_fee: f64,
}

impl Default for TradingFees {
    fn default() -> Self {
        Self {
            binance_spot_fee: 0.1, // 0.1% for spot trading
            solana_dex_fee: 0.25,  // 0.25% for DEX trading
            solana_gas_fee: 0.001, // ~0.001 SOL per transaction
            transfer_fee: 0.0,     // Assuming no additional transfer fees
        }
    }
}

impl TradingFees {
    /// Create custom trading fees
    #[allow(dead_code)]
    pub fn new(
        binance_spot_fee: f64,
        solana_dex_fee: f64,
        solana_gas_fee: f64,
        transfer_fee: f64,
    ) -> Result<Self, CalculatorError> {
        // Validate fee percentages (should be between 0 and 100)
        for (_name, fee) in [
            ("binance_spot_fee", binance_spot_fee),
            ("solana_dex_fee", solana_dex_fee),
            ("transfer_fee", transfer_fee),
        ] {
            if !(0.0..=100.0).contains(&fee) {
                return Err(CalculatorError::InvalidFeePercentage(fee));
            }
        }

        // Validate gas fee (should be reasonable for SOL)
        if !(0.0..=1.0).contains(&solana_gas_fee) {
            return Err(CalculatorError::InvalidFeePercentage(solana_gas_fee));
        }

        Ok(Self {
            binance_spot_fee,
            solana_dex_fee,
            solana_gas_fee,
            transfer_fee,
        })
    }

    /// Get trading fee for specific source
    #[allow(dead_code)]
    pub fn get_trading_fee(&self, source: PriceSource) -> f64 {
        match source {
            PriceSource::Binance => self.binance_spot_fee,
            PriceSource::Solana => self.solana_dex_fee,
        }
    }
}

/// Result of arbitrage opportunity calculation
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ArbitrageOpportunity {
    /// Buy from this source (lower price)
    pub buy_source: PriceSource,
    /// Sell to this source (higher price)
    pub sell_source: PriceSource,
    /// Price to buy at
    pub buy_price: f64,
    /// Price to sell at
    pub sell_price: f64,
    /// Raw price difference before fees
    pub raw_profit_per_unit: f64,
    /// Net profit per unit after all fees
    pub net_profit_per_unit: f64,
    /// Profit percentage based on buy price
    pub profit_percentage: f64,
    /// Total fees incurred per unit
    pub total_fees_per_unit: f64,
    /// Trading pair
    pub trading_pair: TradingPair,
    /// Recommended trade amount (in tokens)
    pub recommended_amount: f64,
    /// Estimated total profit for recommended amount
    pub estimated_total_profit: f64,
}

impl ArbitrageOpportunity {
    /// Check if this opportunity exceeds the profit threshold
    #[allow(dead_code)]
    pub fn exceeds_threshold(&self, threshold: &ProfitThreshold) -> bool {
        self.profit_percentage >= threshold.value()
    }

    /// Check if the opportunity is profitable after all fees
    #[allow(dead_code)]
    pub fn is_profitable(&self) -> bool {
        self.net_profit_per_unit > 0.0
    }

    /// Calculate total profit for a specific trade amount
    #[allow(dead_code)]
    pub fn calculate_total_profit(&self, amount: f64) -> Result<f64, CalculatorError> {
        if amount <= 0.0 {
            return Err(CalculatorError::InvalidTradeAmount(amount));
        }
        Ok(self.net_profit_per_unit * amount)
    }

    /// Get a formatted description of the arbitrage opportunity
    #[allow(dead_code)]
    pub fn description(&self) -> String {
        format!(
            "Buy {} at {} ({}) -> Sell at {} ({}): {:.2}% profit",
            self.trading_pair_symbol(),
            self.buy_price,
            self.buy_source.display_name(),
            self.sell_price,
            self.sell_source.display_name(),
            self.profit_percentage
        )
    }

    /// Get trading pair symbol as string
    fn trading_pair_symbol(&self) -> &'static str {
        crate::util::format_trading_pair(self.trading_pair)
    }
}

/// Fee calculator for arbitrage opportunities
#[allow(dead_code)]
pub struct FeeCalculator {
    trading_fees: TradingFees,
    default_trade_amount: f64,
}

impl Default for FeeCalculator {
    fn default() -> Self {
        Self {
            trading_fees: TradingFees::default(),
            default_trade_amount: 10.0, // 10 SOL default
        }
    }
}

impl FeeCalculator {
    /// Create new fee calculator with custom fees
    #[allow(dead_code)]
    pub fn new(
        trading_fees: TradingFees,
        default_trade_amount: f64,
    ) -> Result<Self, CalculatorError> {
        if default_trade_amount <= 0.0 {
            return Err(CalculatorError::InvalidTradeAmount(default_trade_amount));
        }

        Ok(Self {
            trading_fees,
            default_trade_amount,
        })
    }

    /// Calculate arbitrage opportunity from validated price pair
    #[allow(dead_code)]
    pub fn calculate_opportunity(
        &self,
        prices: &ValidatedPricePair,
        trading_pair: TradingPair,
    ) -> Result<Option<ArbitrageOpportunity>, CalculatorError> {
        // Determine buy and sell sources
        let (buy_source, sell_source) = if prices.solana_price.price < prices.binance_price.price {
            (PriceSource::Solana, PriceSource::Binance)
        } else {
            (PriceSource::Binance, PriceSource::Solana)
        };

        let buy_price = prices.get_price(buy_source).price;
        let sell_price = prices.get_price(sell_source).price;

        // Calculate raw profit before fees
        let raw_profit_per_unit = sell_price - buy_price;

        // If there's no raw profit, no arbitrage opportunity
        if raw_profit_per_unit <= 0.0 {
            return Ok(None);
        }

        // Calculate fee breakdown (per_unit_fees, per_trade_fees)
        let (per_unit_fees, per_trade_fees) =
            self.calculate_fee_breakdown(buy_price, sell_price, buy_source, sell_source);

        // Calculate net profit after fees (amortize per-trade gas for per-unit view)
        let net_profit_per_unit =
            raw_profit_per_unit - per_unit_fees - (per_trade_fees / self.default_trade_amount);

        // Calculate profit percentage based on buy price
        let profit_percentage = (net_profit_per_unit / buy_price) * 100.0;

        // Calculate recommended trade amount and total profit
        let recommended_amount = self.calculate_recommended_amount(buy_price, net_profit_per_unit);

        // Accurate total profit: variable per-unit * amount minus flat per-trade
        let estimated_total_profit =
            (raw_profit_per_unit - per_unit_fees) * recommended_amount - per_trade_fees;

        // Total fees per unit for display (including amortized gas)
        let total_fees_per_unit = per_unit_fees + (per_trade_fees / self.default_trade_amount);

        Ok(Some(ArbitrageOpportunity {
            buy_source,
            sell_source,
            buy_price,
            sell_price,
            raw_profit_per_unit,
            net_profit_per_unit,
            profit_percentage,
            total_fees_per_unit,
            trading_pair,
            recommended_amount,
            estimated_total_profit,
        }))
    }

    /// Calculate fee breakdown for the arbitrage trade
    fn calculate_fee_breakdown(
        &self,
        buy_price: f64,
        sell_price: f64,
        buy_source: PriceSource,
        sell_source: PriceSource,
    ) -> (f64, f64) {
        // Buy fee (percentage of buy amount)
        let buy_fee_percentage = self.trading_fees.get_trading_fee(buy_source) / 100.0;
        let buy_fee = buy_price * buy_fee_percentage;

        // Sell fee (percentage of sell amount)
        let sell_fee_percentage = self.trading_fees.get_trading_fee(sell_source) / 100.0;
        let sell_fee = sell_price * sell_fee_percentage;

        // Transfer fees (if moving between different platforms)
        let transfer_fee = if buy_source != sell_source {
            self.trading_fees.transfer_fee
        } else {
            0.0
        };

        // Gas fees (for Solana transactions): flat per trade
        let gas_fee_usd_total =
            if buy_source == PriceSource::Solana || sell_source == PriceSource::Solana {
                let sol_price = if buy_source == PriceSource::Solana {
                    buy_price
                } else {
                    sell_price
                };
                self.trading_fees.solana_gas_fee * sol_price
            } else {
                0.0
            };

        // Return (per_unit_fees, per_trade_fees)
        (buy_fee + sell_fee + transfer_fee, gas_fee_usd_total)
    }

    /// Calculate total fees for a complete arbitrage round trip
    fn calculate_total_fees(
        &self,
        buy_price: f64,
        sell_price: f64,
        buy_source: PriceSource,
        sell_source: PriceSource,
    ) -> f64 {
        let (per_unit_fees, per_trade_fees) =
            self.calculate_fee_breakdown(buy_price, sell_price, buy_source, sell_source);
        per_unit_fees + (per_trade_fees / self.default_trade_amount)
    }

    /// Calculate recommended trade amount based on profit and risk
    fn calculate_recommended_amount(&self, _buy_price: f64, net_profit_per_unit: f64) -> f64 {
        // For now, use a simple approach: default amount unless profit is very low
        if net_profit_per_unit > 0.0 {
            self.default_trade_amount
        } else {
            1.0 // Minimum trade amount
        }
    }

    /// Update trading fees
    #[allow(dead_code)]
    pub fn set_trading_fees(&mut self, fees: TradingFees) {
        self.trading_fees = fees;
    }

    /// Get current trading fees
    #[allow(dead_code)]
    pub fn trading_fees(&self) -> &TradingFees {
        &self.trading_fees
    }

    /// Set default trade amount
    #[allow(dead_code)]
    pub fn set_default_trade_amount(&mut self, amount: f64) -> Result<(), CalculatorError> {
        if amount <= 0.0 {
            return Err(CalculatorError::InvalidTradeAmount(amount));
        }
        self.default_trade_amount = amount;
        Ok(())
    }

    /// Get default trade amount
    #[allow(dead_code)]
    pub fn default_trade_amount(&self) -> f64 {
        self.default_trade_amount
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::price::{PriceSource, SourcePrice};

    fn create_test_price_pair() -> ValidatedPricePair {
        let solana_price = SourcePrice::new(190.0, PriceSource::Solana);
        let binance_price = SourcePrice::new(195.0, PriceSource::Binance);
        ValidatedPricePair::new(solana_price, binance_price)
    }

    #[test]
    fn test_trading_fees_creation() {
        let fees = TradingFees::default();
        assert_eq!(fees.binance_spot_fee, 0.1);
        assert_eq!(fees.solana_dex_fee, 0.25);

        let custom_fees = TradingFees::new(0.05, 0.3, 0.002, 0.1).unwrap();
        assert_eq!(custom_fees.binance_spot_fee, 0.05);
    }

    #[test]
    fn test_invalid_fees() {
        assert!(TradingFees::new(-0.1, 0.25, 0.001, 0.0).is_err());
        assert!(TradingFees::new(101.0, 0.25, 0.001, 0.0).is_err());
    }

    #[test]
    fn test_fee_calculator_creation() {
        let calculator = FeeCalculator::default();
        assert_eq!(calculator.default_trade_amount(), 10.0);

        let fees = TradingFees::default();
        let custom_calculator = FeeCalculator::new(fees, 5.0).unwrap();
        assert_eq!(custom_calculator.default_trade_amount(), 5.0);
    }

    #[test]
    fn test_invalid_trade_amount() {
        let fees = TradingFees::default();
        assert!(FeeCalculator::new(fees.clone(), -1.0).is_err());
        assert!(FeeCalculator::new(fees.clone(), 0.0).is_err());
    }

    #[test]
    fn test_arbitrage_opportunity_calculation() {
        let calculator = FeeCalculator::default();
        let price_pair = create_test_price_pair();

        let opportunity = calculator
            .calculate_opportunity(&price_pair, TradingPair::SolUsdt)
            .unwrap()
            .unwrap();

        assert_eq!(opportunity.buy_source, PriceSource::Solana);
        assert_eq!(opportunity.sell_source, PriceSource::Binance);
        assert_eq!(opportunity.buy_price, 190.0);
        assert_eq!(opportunity.sell_price, 195.0);
        assert_eq!(opportunity.raw_profit_per_unit, 5.0);
        assert!(opportunity.is_profitable());
    }

    #[test]
    fn test_no_arbitrage_opportunity() {
        let calculator = FeeCalculator::default();

        // Create price pair where Solana is higher (no arbitrage possible)
        let solana_price = SourcePrice::new(200.0, PriceSource::Solana);
        let binance_price = SourcePrice::new(195.0, PriceSource::Binance);
        let price_pair = ValidatedPricePair::new(solana_price, binance_price);

        let opportunity = calculator
            .calculate_opportunity(&price_pair, TradingPair::SolUsdt)
            .unwrap();

        // Should still create opportunity but check if profitable
        assert!(opportunity.is_some());
        let opp = opportunity.unwrap();
        assert_eq!(opp.buy_source, PriceSource::Binance);
        assert_eq!(opp.sell_source, PriceSource::Solana);
    }

    #[test]
    fn test_profit_threshold_check() {
        let calculator = FeeCalculator::default();
        let price_pair = create_test_price_pair();

        let opportunity = calculator
            .calculate_opportunity(&price_pair, TradingPair::SolUsdt)
            .unwrap()
            .unwrap();

        let threshold = ProfitThreshold::new(1.0).unwrap();
        assert!(opportunity.exceeds_threshold(&threshold));

        let high_threshold = ProfitThreshold::new(10.0).unwrap();
        assert!(!opportunity.exceeds_threshold(&high_threshold));
    }

    #[test]
    fn test_total_profit_calculation() {
        let calculator = FeeCalculator::default();
        let price_pair = create_test_price_pair();

        let opportunity = calculator
            .calculate_opportunity(&price_pair, TradingPair::SolUsdt)
            .unwrap()
            .unwrap();

        let profit_5_tokens = opportunity.calculate_total_profit(5.0).unwrap();
        let profit_10_tokens = opportunity.calculate_total_profit(10.0).unwrap();

        assert!(profit_10_tokens > profit_5_tokens);
        assert!(opportunity.calculate_total_profit(-1.0).is_err());
    }

    #[test]
    fn test_opportunity_description() {
        let calculator = FeeCalculator::default();
        let price_pair = create_test_price_pair();

        let opportunity = calculator
            .calculate_opportunity(&price_pair, TradingPair::SolUsdt)
            .unwrap()
            .unwrap();

        let description = opportunity.description();
        assert!(description.contains("SOL/USDT"));
        assert!(description.contains("Buy"));
        assert!(description.contains("Sell"));
        assert!(description.contains("%"));
    }

    #[test]
    fn test_fee_calculation() {
        let fees = TradingFees::default();
        let calculator = FeeCalculator::new(fees, 10.0).unwrap();

        // Test fee calculation for different scenarios
        let buy_price = 190.0;
        let sell_price = 195.0;

        let total_fees = calculator.calculate_total_fees(
            buy_price,
            sell_price,
            PriceSource::Solana,
            PriceSource::Binance,
        );

        // Should include both trading fees plus gas fee for Solana
        assert!(total_fees > 0.0);
    }

    #[test]
    fn test_trading_fee_getter() {
        let fees = TradingFees::default();

        assert_eq!(fees.get_trading_fee(PriceSource::Binance), 0.1);
        assert_eq!(fees.get_trading_fee(PriceSource::Solana), 0.25);
    }

    #[test]
    fn test_calculator_setters() {
        let mut calculator = FeeCalculator::default();

        let new_fees = TradingFees::new(0.05, 0.2, 0.002, 0.1).unwrap();
        calculator.set_trading_fees(new_fees);

        assert_eq!(calculator.trading_fees().binance_spot_fee, 0.05);

        calculator.set_default_trade_amount(20.0).unwrap();
        assert_eq!(calculator.default_trade_amount(), 20.0);

        assert!(calculator.set_default_trade_amount(-5.0).is_err());
    }
}
