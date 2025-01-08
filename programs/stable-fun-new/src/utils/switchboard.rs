use anchor_lang::prelude::*;
use switchboard_solana::{
    AggregatorAccountData,
    SWITCHBOARD_PROGRAM_ID,
};
use crate::error::StableFunError;

#[derive(Clone)]
pub struct PriceData {
    pub price: u64,
    pub timestamp: i64,
}

#[inline(never)]
pub fn get_validated_price(
    feed: &AccountLoader<AggregatorAccountData>,
    max_staleness: i64,
) -> Result<u64> {
    let feed_data = feed.load()?;
    
    // Get the latest result
    let result = feed_data.get_result()
        .map_err(|_| error!(StableFunError::InvalidOraclePrice))?;

    // Validate price
    require!(result.mantissa > 0, StableFunError::InvalidOraclePrice);
    
    // Check staleness
    let current_timestamp = Clock::get()?.unix_timestamp;
    let last_timestamp = feed_data.latest_confirmed_round.round_open_timestamp;
    require!(
        current_timestamp - last_timestamp <= max_staleness,
        StableFunError::StaleOraclePrice
    );
    
    Ok(result.mantissa as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    // Tests will be updated for v3
    #[test]
    fn test_price_validation() {
        // Test implementations will go here
    }
}