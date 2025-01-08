use anchor_lang::prelude::*;
use switchboard_solana::AggregatorAccountData;
use crate::error::StableFunError;

// Constants
pub const MAX_PRICE_STALENESS: i64 = 300; // 5 minutes
pub const PRICE_DECIMALS: u8 = 6;
pub const MAX_ORACLE_CONFIDENCE: u64 = 100_000; // 1% of base price
pub const MIN_ORACLE_COUNT: usize = 1;
pub const MAX_ORACLE_COUNT: usize = 3;

#[derive(Clone, Debug)]
pub struct OraclePrice {
    pub value: u64,
    pub decimals: u8,
    pub last_updated: i64,
    pub confidence: u64,
}

impl OraclePrice {
    #[inline(always)]
    pub fn new(value: u64, decimals: u8, last_updated: i64, confidence: u64) -> Self {
        Self {
            value,
            decimals,
            last_updated,
            confidence,
        }
    }

    #[inline(always)]
    pub fn from_switchboard(oracle: &AggregatorAccountData) -> Result<Self> {
        let result = oracle.get_result()
            .map_err(|_| error!(StableFunError::InvalidOraclePrice))?;

        Ok(Self {
            value: result.mantissa as u64,
            decimals: result.scale as u8,
            last_updated: oracle.latest_confirmed_round.round_open_timestamp,
            confidence: result.mantissa as u64,
        })
    }

    #[inline(always)]
    pub fn is_stale(&self, current_timestamp: i64) -> bool {
        current_timestamp.saturating_sub(self.last_updated) > MAX_PRICE_STALENESS
    }

    #[inline(always)]
    pub fn standardize(&self) -> Result<u64> {
        let current_decimals = self.decimals;
        let target_decimals = PRICE_DECIMALS;

        match current_decimals.cmp(&target_decimals) {
            std::cmp::Ordering::Equal => Ok(self.value),
            std::cmp::Ordering::Greater => {
                let diff = current_decimals - target_decimals;
                self.value
                    .checked_div(10u64.pow(diff as u32))
                    .ok_or(error!(StableFunError::MathOverflow))
            }
            std::cmp::Ordering::Less => {
                let diff = target_decimals - current_decimals;
                self.value
                    .checked_mul(10u64.pow(diff as u32))
                    .ok_or(error!(StableFunError::MathOverflow))
            }
        }
    }
}

pub struct OracleService;

impl OracleService {
    #[inline(always)]
    pub fn get_price(oracle_account: &AccountLoader<AggregatorAccountData>) -> Result<OraclePrice> {
        let oracle = oracle_account.load()?;
        
        require!(
            oracle.latest_confirmed_round.round_open_timestamp > 0,
            StableFunError::InvalidOracle
        );

        OraclePrice::from_switchboard(&oracle)
    }

    #[inline(always)]
    pub fn validate_price(
        price: &OraclePrice,
        max_confidence_interval: Option<u64>,
    ) -> Result<()> {
        require!(price.value > 0, StableFunError::InvalidOraclePrice);

        let clock = Clock::get()?;
        require!(
            !price.is_stale(clock.unix_timestamp),
            StableFunError::StaleOraclePrice
        );

        let max_confidence = max_confidence_interval.unwrap_or(MAX_ORACLE_CONFIDENCE);
        require!(
            price.confidence <= max_confidence,
            StableFunError::InvalidOraclePrice
        );

        Ok(())
    }

    pub fn verify_oracle_price(
        feed: &AccountLoader<AggregatorAccountData>
    ) -> Result<u64> {
        let price = Self::get_price(feed)?;
        Self::validate_price(&price, None)?;
        price.standardize()
    }

    #[inline(always)]
    pub fn get_median_price(
        oracle_accounts: &[AccountLoader<AggregatorAccountData>]
    ) -> Result<OraclePrice> {
        require!(
            (MIN_ORACLE_COUNT..=MAX_ORACLE_COUNT).contains(&oracle_accounts.len()),
            StableFunError::InvalidOracle
        );

        let mut prices = Vec::with_capacity(MAX_ORACLE_COUNT);

        for oracle_account in oracle_accounts.iter().take(MAX_ORACLE_COUNT) {
            if let Ok(price) = Self::get_price(oracle_account) {
                if Self::validate_price(&price, None).is_ok() {
                    prices.push(price);
                }
            }
        }

        require!(!prices.is_empty(), StableFunError::InvalidOraclePrice);
        prices.sort_by(|a, b| a.value.cmp(&b.value));
        Ok(prices[prices.len() / 2].clone())
    }

    #[inline(always)]
    pub fn calculate_safe_price(
        price: &OraclePrice,
        is_upper_bound: bool,
    ) -> Result<u64> {
        let base_price = price.standardize()?;
        
        if is_upper_bound {
            base_price
                .checked_add(price.confidence)
                .ok_or(error!(StableFunError::MathOverflow))
        } else {
            base_price
                .checked_sub(price.confidence)
                .ok_or(error!(StableFunError::MathOverflow))
        }
    }

    #[inline(always)]
    pub fn aggregate_price(
        oracle_accounts: &[AccountLoader<AggregatorAccountData>],
        is_upper_bound: bool,
    ) -> Result<u64> {
        let median_price = Self::get_median_price(oracle_accounts)?;
        Self::calculate_safe_price(&median_price, is_upper_bound)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_price_standardization() {
        let price = OraclePrice::new(1_000_000_000, 9, 0, 0);
        assert_eq!(price.standardize().unwrap(), 1_000_000);

        let price = OraclePrice::new(1_000, 3, 0, 0);
        assert_eq!(price.standardize().unwrap(), 1_000_000);
    }

    #[test]
    fn test_price_staleness() {
        let price = OraclePrice::new(1_000_000, 6, 1000, 0);
        assert!(price.is_stale(1500));
        assert!(!price.is_stale(1200));
    }

    #[test]
    fn test_safe_price_calculation() {
        let price = OraclePrice::new(1_000_000, 6, 0, 1000);
        assert_eq!(
            OracleService::calculate_safe_price(&price, true).unwrap(),
            1_001_000
        );
        assert_eq!(
            OracleService::calculate_safe_price(&price, false).unwrap(),
            999_000
        );
    }
}