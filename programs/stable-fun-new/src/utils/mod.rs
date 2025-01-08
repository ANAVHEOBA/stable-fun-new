pub mod math;
pub mod oracle;
pub mod stablebond;
pub mod token;
pub mod validation;
pub mod switchboard;

use anchor_lang::prelude::*;
use switchboard_solana::{
    AggregatorAccountData,
    SWITCHBOARD_PROGRAM_ID,
};
use crate::error::StableFunError;

pub const PRICE_DECIMALS: u8 = 6;
pub const MAX_PRICE_STALENESS: i64 = 300; // 5 minutes
pub const BASIS_POINTS_DIVISOR: u16 = 10000;
pub const MINIMUM_LIQUIDITY: u64 = 1000;

/// Common utility functions
pub mod common {
    use super::*;

    #[inline(always)]
    pub fn get_current_timestamp() -> Result<i64> {
        Ok(Clock::get()?.unix_timestamp)
    }

    #[inline(always)]
    pub fn basis_points_to_decimal(basis_points: u16) -> Result<f64> {
        Ok(basis_points as f64 / BASIS_POINTS_DIVISOR as f64)
    }

    #[inline(always)]
    pub fn calculate_percentage(amount: u64, basis_points: u16) -> Result<u64> {
        amount
            .checked_mul(basis_points as u64)
            .and_then(|v| v.checked_div(BASIS_POINTS_DIVISOR as u64))
            .ok_or_else(|| error!(StableFunError::MathOverflow))
    }

    #[inline(always)]
    pub fn verify_account_owner(account: &AccountInfo, owner: &Pubkey) -> Result<()> {
        require!(
            account.owner == owner,
            StableFunError::AccountOwnerMismatch
        );
        Ok(())
    }
}

/// Oracle price validation and conversion
pub mod oracle_utils {
    use super::*;
    use std::cell::Ref;

    #[inline(always)]
    pub fn get_validated_price(oracle_account: &AccountLoader<AggregatorAccountData>) -> Result<u64> {
        let oracle = oracle_account.load()?;
        let latest_round = oracle.latest_confirmed_round;
        
        require!(latest_round.result.mantissa > 0, StableFunError::InvalidOraclePrice);

        let current_timestamp = Clock::get()?.unix_timestamp;
        let last_update = latest_round.round_open_timestamp;

        require!(
            current_timestamp - last_update <= MAX_PRICE_STALENESS,
            StableFunError::StaleOraclePrice
        );

        Ok(latest_round.result.mantissa as u64)
    }

    #[inline(always)]
    pub fn is_price_stale(last_update: i64) -> Result<bool> {
        let current_timestamp = Clock::get()?.unix_timestamp;
        Ok(current_timestamp - last_update > MAX_PRICE_STALENESS)
    }
}

/// PDA derivation utilities
pub mod pda {
    use super::*;

    #[inline(always)]
    pub fn find_stablecoin_mint_address(
        program_id: &Pubkey,
        authority: &Pubkey,
        symbol: &str,
    ) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[
                b"stablecoin",
                authority.as_ref(),
                symbol.as_bytes(),
            ],
            program_id,
        )
    }

    #[inline(always)]
    pub fn find_vault_address(
        program_id: &Pubkey,
        stablecoin_mint: &Pubkey,
    ) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[
                b"vault",
                stablecoin_mint.as_ref(),
            ],
            program_id,
        )
    }

    #[inline(always)]
    pub fn find_mint_authority_address(
        program_id: &Pubkey,
        stablecoin_mint: &Pubkey,
    ) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[
                b"mint-authority",
                stablecoin_mint.as_ref(),
            ],
            program_id,
        )
    }
}

// Re-export commonly used functions
pub use common::*;
pub use oracle_utils::*;
pub use pda::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basis_points_conversion() {
        let result = common::basis_points_to_decimal(500).unwrap();
        assert_eq!(result, 0.05);

        let result = common::basis_points_to_decimal(1000).unwrap();
        assert_eq!(result, 0.10);
    }

    #[test]
    fn test_percentage_calculation() {
        let amount = 1_000_000;
        let basis_points = 500; // 5%

        let result = common::calculate_percentage(amount, basis_points).unwrap();
        assert_eq!(result, 50_000);
    }

    #[test]
    fn test_pda_derivation() {
        let program_id = Pubkey::new_unique();
        let authority = Pubkey::new_unique();
        let symbol = "TEST";

        let (mint_address, _bump) = pda::find_stablecoin_mint_address(
            &program_id,
            &authority,
            symbol,
        );
        assert_ne!(mint_address, Pubkey::default());
    }
}