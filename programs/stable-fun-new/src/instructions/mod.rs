pub mod initialize;
pub mod mint;
pub mod redeem;
pub mod update;

pub use initialize::*;
pub use mint::*;
pub use redeem::*;
pub use update::*;

use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};
use switchboard_solana::{
    AggregatorAccountData,
    SWITCHBOARD_PROGRAM_ID,
};

use crate::utils::switchboard::get_validated_price;

/// Seeds for PDA derivation
pub const STABLECOIN_SEED: &[u8] = b"stablecoin";
pub const VAULT_SEED: &[u8] = b"vault";
pub const MINT_AUTHORITY_SEED: &[u8] = b"mint-authority";

/// Constants for validation
pub const MIN_NAME_LENGTH: usize = 3;
pub const MIN_SYMBOL_LENGTH: usize = 2;
pub const BASIS_POINTS_DIVISOR: u16 = 10000;
pub const DEFAULT_COLLATERAL_RATIO: u16 = 15000; // 150%
pub const MIN_COLLATERAL_RATIO: u16 = 10000; // 100%

/// Helper function to verify oracle price data
#[inline(never)]
pub fn verify_oracle_price(
    oracle_account: &AccountLoader<AggregatorAccountData>,
) -> Result<u64> {
    get_validated_price(oracle_account, 300) // 5 minutes staleness
}

/// Helper function to calculate token amounts based on price
#[inline(never)]
pub fn calculate_token_amount(
    amount: u64,
    price: u64,
    decimals: u8,
) -> Result<u64> {
    let scale = 10u64.pow(decimals as u32);
    amount
        .checked_mul(scale)
        .and_then(|a| a.checked_div(price))
        .ok_or(ProgramError::MathOverflow.into())
}

/// Helper function to validate collateral ratio
#[inline(never)]
pub fn validate_collateral_ratio(
    collateral_amount: u64,
    collateral_value: u64,
    min_ratio: u16,
) -> Result<()> {
    let ratio = collateral_value
        .checked_mul(BASIS_POINTS_DIVISOR as u64)
        .and_then(|v| v.checked_div(collateral_amount))
        .ok_or(ProgramError::MathOverflow)?;

    require!(
        ratio >= min_ratio as u64,
        ProgramError::InsufficientCollateral
    );

    Ok(())
}

/// Helper function to transfer tokens
#[inline(never)]
pub fn transfer_tokens<'info>(
    from: &Account<'info, TokenAccount>,
    to: &Account<'info, TokenAccount>,
    authority: &Signer<'info>,
    token_program: &Program<'info, Token>,
    amount: u64,
) -> Result<()> {
    token::transfer(
        CpiContext::new(
            token_program.to_account_info(),
            Transfer {
                from: from.to_account_info(),
                to: to.to_account_info(),
                authority: authority.to_account_info(),
            },
        ),
        amount,
    )
}

#[error_code]
pub enum ProgramError {
    #[msg("Invalid oracle price")]
    InvalidOraclePrice,
    #[msg("Stale oracle price")]
    StaleOraclePrice,
    #[msg("Math overflow in calculation")]
    MathOverflow,
    #[msg("Insufficient collateral ratio")]
    InsufficientCollateral,
}