use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, TokenAccount};

use crate::error::StableFunError;
use crate::state::{StablecoinMint, StablecoinVault};
use crate::utils::oracle::OraclePrice;

// Constants for validation
pub const MIN_TRANSACTION_AMOUNT: u64 = 1_000;
pub const MAX_TRANSACTION_AMOUNT: u64 = 1_000_000_000_000;
pub const MIN_COLLATERAL_RATIO_BPS: u16 = 10000; // 100%
pub const MAX_COLLATERAL_RATIO_BPS: u16 = 30000; // 300%
pub const MAX_FEE_BPS: u16 = 1000; // 10%
pub const MIN_NAME_LENGTH: usize = 3;
pub const MAX_NAME_LENGTH: usize = 32;
pub const MIN_SYMBOL_LENGTH: usize = 2;
pub const MAX_SYMBOL_LENGTH: usize = 10;

#[derive(Default)]
pub struct ValidationService;

impl ValidationService {
    #[inline(always)]
    pub fn validate_amount(amount: u64) -> Result<()> {
        require!(
            (MIN_TRANSACTION_AMOUNT..=MAX_TRANSACTION_AMOUNT).contains(&amount),
            StableFunError::AmountTooSmall
        );
        Ok(())
    }

    #[inline(always)]
    pub fn validate_collateral_ratio(
        collateral: u64,
        supply: u64,
        min_ratio: u16,
    ) -> Result<()> {
        if supply == 0 {
            return Ok(());
        }

        let ratio = (collateral as u128)
            .checked_mul(10000)
            .and_then(|v| v.checked_div(supply as u128))
            .map(|v| v as u16)
            .ok_or(error!(StableFunError::MathOverflow))?;

        require!(
            (min_ratio..=MAX_COLLATERAL_RATIO_BPS).contains(&ratio),
            StableFunError::CollateralRatioTooLow
        );

        Ok(())
    }

    #[inline(always)]
    pub fn update_collateral_ratio(vault: &mut Account<StablecoinVault>) -> Result<()> {
        if vault.total_value_locked == 0 || vault.total_collateral == 0 {
            vault.current_ratio = 0;
            return Ok(());
        }

        let ratio = (vault.total_value_locked as u128)
            .checked_mul(10000)
            .and_then(|v| v.checked_div(vault.total_collateral as u128))
            .map(|v| v as u16)
            .ok_or(error!(StableFunError::MathOverflow))?;

        vault.current_ratio = ratio;
        Ok(())
    }

    #[inline(always)]
    pub fn validate_fee(fee_bps: u16) -> Result<()> {
        require!(fee_bps <= MAX_FEE_BPS, StableFunError::FeeTooHigh);
        Ok(())
    }

    #[inline(always)]
    pub fn validate_metadata(
        name: &str,
        symbol: &str,
        currency: &str,
    ) -> Result<()> {
        require!(
            (MIN_NAME_LENGTH..=MAX_NAME_LENGTH).contains(&name.len()),
            StableFunError::InvalidName
        );

        require!(
            (MIN_SYMBOL_LENGTH..=MAX_SYMBOL_LENGTH).contains(&symbol.len()),
            StableFunError::InvalidSymbol
        );

        require!(
            !currency.is_empty() && currency.len() <= 5,
            StableFunError::InvalidCurrency
        );

        Ok(())
    }

    #[inline(always)]
    pub fn validate_token_accounts(
        mint: &Account<Mint>,
        token_account: &Account<TokenAccount>,
        owner: &Pubkey,
    ) -> Result<()> {
        require!(
            token_account.mint == mint.key() && token_account.owner == *owner,
            StableFunError::InvalidTokenAccount
        );
        Ok(())
    }

    #[inline(always)]
    pub fn validate_vault_state(
        vault: &Account<StablecoinVault>,
        stablecoin_mint: &Account<StablecoinMint>,
    ) -> Result<()> {
        require!(
            vault.stablecoin_mint == stablecoin_mint.key() && vault.total_collateral > 0,
            StableFunError::InvalidVault
        );
        Ok(())
    }

    #[inline(always)]
    pub fn validate_mint_operation(
        stablecoin_mint: &Account<StablecoinMint>,
        amount: u64,
        oracle_price: &OraclePrice,
        current_collateral: u64,
    ) -> Result<()> {
        require!(!stablecoin_mint.settings.mint_paused, StableFunError::MintingPaused);
        Self::validate_amount(amount)?;

        let new_supply = stablecoin_mint
            .current_supply
            .checked_add(amount)
            .ok_or(error!(StableFunError::MathOverflow))?;
            
        require!(
            new_supply <= stablecoin_mint.settings.max_supply,
            StableFunError::MaxSupplyExceeded
        );

        require!(oracle_price.value > 0, StableFunError::InvalidOraclePrice);

        Self::validate_collateral_ratio(
            current_collateral,
            new_supply,
            stablecoin_mint.settings.min_collateral_ratio,
        )?;

        Ok(())
    }

    #[inline(always)]
    pub fn validate_redeem_operation(
        stablecoin_mint: &Account<StablecoinMint>,
        vault: &Account<StablecoinVault>,
        amount: u64,
        token_account: &Account<TokenAccount>,
        remaining_collateral: u64,
    ) -> Result<()> {
        require!(!stablecoin_mint.settings.redeem_paused, StableFunError::RedeemingPaused);
        Self::validate_amount(amount)?;

        require!(
            token_account.amount >= amount && vault.total_collateral > 0,
            StableFunError::InsufficientBalance
        );

        let new_supply = stablecoin_mint
            .current_supply
            .checked_sub(amount)
            .ok_or(error!(StableFunError::MathOverflow))?;

        Self::validate_collateral_ratio(
            remaining_collateral,
            new_supply,
            stablecoin_mint.settings.min_collateral_ratio,
        )?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_amount_validation() {
        assert!(ValidationService::validate_amount(MIN_TRANSACTION_AMOUNT).is_ok());
        assert!(ValidationService::validate_amount(MIN_TRANSACTION_AMOUNT - 1).is_err());
        assert!(ValidationService::validate_amount(MAX_TRANSACTION_AMOUNT).is_ok());
        assert!(ValidationService::validate_amount(MAX_TRANSACTION_AMOUNT + 1).is_err());
    }

    #[test]
    fn test_collateral_ratio_validation() {
        assert!(ValidationService::validate_collateral_ratio(
            15000000, // 150% collateral
            10000000, // supply
            10000     // min ratio 100%
        ).is_ok());

        assert!(ValidationService::validate_collateral_ratio(
            9000000,  // 90% collateral
            10000000, // supply
            10000     // min ratio 100%
        ).is_err());
    }

    #[test]
    fn test_metadata_validation() {
        assert!(ValidationService::validate_metadata(
            "Test Coin",
            "TEST",
            "USD"
        ).is_ok());
        
        assert!(ValidationService::validate_metadata(
            "Te",  // Too short
            "TEST",
            "USD"
        ).is_err());
        
        assert!(ValidationService::validate_metadata(
            "Test Coin",
            "T",   // Too short
            "USD"
        ).is_err());
    }

    #[test]
    fn test_fee_validation() {
        assert!(ValidationService::validate_fee(500).is_ok()); // 5%
        assert!(ValidationService::validate_fee(1100).is_err()); // 11%
    }
}