use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount};

// Define the error enum
#[error_code]
pub enum StablebondError {
    #[msg("Stablebond has matured")]
    StablebondMatured,
    #[msg("Invalid yield rate")]
    InvalidYieldRate,
    #[msg("Invalid stablebond")]
    InvalidStablebond,
    #[msg("Math overflow")]
    MathOverflow,
}

// Define the account structure
#[account]
#[derive(Debug)]
pub struct StablebondMint {
    pub authority: Pubkey,
    pub underlying_mint: Pubkey,
    pub current_yield: u64,
    pub maturity_timestamp: i64,
    pub supply: u64,
    pub decimals: u8,
    pub last_yield_update: i64,
    pub next_yield_update: i64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct YieldData {
    pub current_yield: u64,
    pub last_update: i64,
    pub next_update: i64,
}

/// Constants for stablebond operations
pub const MIN_BOND_DURATION: i64 = 24 * 60 * 60; // 1 day in seconds
pub const MAX_BOND_DURATION: i64 = 365 * 24 * 60 * 60; // 1 year in seconds
pub const YIELD_DECIMALS: u8 = 6;

/// Struct to hold stablebond data
#[derive(Debug)]
pub struct StablebondData {
    pub mint: Pubkey,
    pub underlying_mint: Pubkey,
    pub current_yield: u64,
    pub maturity_timestamp: i64,
    pub total_supply: u64,
    pub decimals: u8,
}

/// Service for interacting with stablebonds
pub struct StablebondService;

impl StablebondService {
    /// Get stablebond data
    pub fn get_stablebond_data(
        stablebond_mint: &Account<StablebondMint>,
    ) -> Result<StablebondData> {
        Ok(StablebondData {
            mint: stablebond_mint.key(),
            underlying_mint: stablebond_mint.underlying_mint,
            current_yield: stablebond_mint.current_yield,
            maturity_timestamp: stablebond_mint.maturity_timestamp,
            total_supply: stablebond_mint.supply,
            decimals: stablebond_mint.decimals,
        })
    }

    /// Validate stablebond for use as collateral
    pub fn validate_stablebond(
        stablebond_mint: &Account<StablebondMint>,
        current_timestamp: i64,
    ) -> Result<()> {
        // Check maturity
        require!(
            stablebond_mint.maturity_timestamp > current_timestamp,
            StablebondError::StablebondMatured
        );

        // Check yield rate
        require!(
            stablebond_mint.current_yield > 0,
            StablebondError::InvalidYieldRate
        );

        // Check supply
        require!(
            stablebond_mint.supply > 0,
            StablebondError::InvalidStablebond
        );

        Ok(())
    }

    /// Calculate current value of stablebond holdings
    pub fn calculate_value(
        amount: u64,
        stablebond: &StablebondData,
        price: u64,
    ) -> Result<u64> {
        let base_value = amount
            .checked_mul(price)
            .ok_or(StablebondError::MathOverflow)?
            .checked_div(10u64.pow(stablebond.decimals as u32))
            .ok_or(StablebondError::MathOverflow)?;

        // Add accrued yield
        let yield_value = Self::calculate_accrued_yield(amount, stablebond)?;
        
        base_value
            .checked_add(yield_value)
            .ok_or(StablebondError::MathOverflow.into())
    }

    /// Calculate accrued yield
    pub fn calculate_accrued_yield(
        amount: u64,
        stablebond: &StablebondData,
    ) -> Result<u64> {
        let current_timestamp = Clock::get()?.unix_timestamp;
        let time_to_maturity = stablebond
            .maturity_timestamp
            .checked_sub(current_timestamp)
            .ok_or(StablebondError::MathOverflow)?;

        // Calculate yield based on remaining time
        let yield_amount = amount
            .checked_mul(stablebond.current_yield)
            .and_then(|v| v.checked_mul(time_to_maturity as u64))
            .and_then(|v| v.checked_div(365 * 24 * 60 * 60)) // Annualized yield
            .and_then(|v| v.checked_div(10u64.pow(YIELD_DECIMALS as u32)))
            .ok_or(StablebondError::MathOverflow)?;

        Ok(yield_amount)
    }

    /// Transfer stablebonds between accounts
    pub fn transfer_stablebonds<'info>(
        from: &Account<'info, TokenAccount>,
        to: &Account<'info, TokenAccount>,
        authority: &Signer<'info>,
        token_program: &Program<'info, Token>,
        amount: u64,
    ) -> Result<()> {
        token::transfer(
            CpiContext::new(
                token_program.to_account_info(),
                token::Transfer {
                    from: from.to_account_info(),
                    to: to.to_account_info(),
                    authority: authority.to_account_info(),
                },
            ),
            amount,
        )?;
        Ok(())
    }

    /// Get yield data for a stablebond
    pub fn get_yield_data(
        stablebond_mint: &Account<StablebondMint>,
    ) -> Result<YieldData> {
        Ok(YieldData {
            current_yield: stablebond_mint.current_yield,
            last_update: stablebond_mint.last_yield_update,
            next_update: stablebond_mint.next_yield_update,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Add test helper function
    fn create_test_stablebond() -> StablebondData {
        StablebondData {
            mint: Pubkey::new_unique(),
            underlying_mint: Pubkey::new_unique(),
            current_yield: 500_000, // 5% APY (6 decimals)
            maturity_timestamp: 1735689600, // 2025-01-01
            total_supply: 1_000_000,
            decimals: 6,
        }
    }

    #[test]
    fn test_value_calculation() {
        let stablebond = create_test_stablebond();
        let price = 1_000_000; // $1.00 (6 decimals)
        let amount = 1_000_000; // 1 token

        let value = StablebondService::calculate_value(
            amount,
            &stablebond,
            price,
        ).unwrap();

        assert!(value > amount); // Value should include yield
    }

    #[test]
    fn test_yield_calculation() {
        let stablebond = create_test_stablebond();
        let amount = 1_000_000; // 1 token

        let yield_amount = StablebondService::calculate_accrued_yield(
            amount,
            &stablebond,
        ).unwrap();

        assert!(yield_amount > 0);
        assert!(yield_amount < amount); // Yield should be less than principal
    }
}