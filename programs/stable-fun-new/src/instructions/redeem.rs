use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount};
use switchboard_solana::AggregatorAccountData;

use crate::state::{StablecoinMint, StablecoinVault};
use crate::error::StableFunError;
use crate::utils::oracle::OracleService;
use crate::utils::validation::ValidationService;
use crate::utils::math;

#[derive(Accounts)]
#[instruction(amount: u64)]
pub struct RedeemStablecoin<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(mut)]
    pub stablecoin_mint: Account<'info, StablecoinMint>,

    #[account(
        mut,
        seeds = [b"vault", stablecoin_mint.key().as_ref()],
        bump,
        constraint = vault.stablecoin_mint == stablecoin_mint.key() @ StableFunError::InvalidVault
    )]
    pub vault: Account<'info, StablecoinVault>,

    #[account(
        mut,
        constraint = token_mint.key() == stablecoin_mint.token_mint @ StableFunError::InvalidMint
    )]
    pub token_mint: Box<Account<'info, token::Mint>>,

    #[account(
        mut,
        constraint = user_token_account.mint == token_mint.key() @ StableFunError::InvalidTokenAccount,
        constraint = user_token_account.owner == user.key() @ StableFunError::InvalidTokenAccount
    )]
    pub user_token_account: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = user_stablebond_account.mint == stablecoin_mint.stablebond_mint @ StableFunError::InvalidStablebond,
        constraint = user_stablebond_account.owner == user.key() @ StableFunError::InvalidStablebond
    )]
    pub user_stablebond_account: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        constraint = vault_stablebond_account.key() == vault.collateral_account @ StableFunError::InvalidVaultAccount
    )]
    pub vault_stablebond_account: Box<Account<'info, TokenAccount>>,

    /// The Switchboard V3 aggregator account
    #[account(
        constraint = price_feed.key() == stablecoin_mint.price_feed @ StableFunError::InvalidOracle
    )]
    pub price_feed: AccountLoader<'info, AggregatorAccountData>,

    /// CHECK: PDA used as burn authority
    #[account(
        seeds = [b"mint-authority", stablecoin_mint.key().as_ref()],
        bump
    )]
    pub burn_authority: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[inline(never)]
pub fn handler(ctx: Context<RedeemStablecoin>, amount: u64) -> Result<()> {
    // Initial validations
    require!(!ctx.accounts.stablecoin_mint.settings.redeem_paused, StableFunError::RedeemingPaused);
    require!(amount > 0, StableFunError::InvalidAmount);
    require!(
        amount <= ctx.accounts.user_token_account.amount,
        StableFunError::InsufficientBalance
    );

    // Validate amount is within bounds
    ValidationService::validate_amount(amount)?;

    // Get oracle price
    let oracle_price = OracleService::verify_oracle_price(&ctx.accounts.price_feed)?;

    // Calculate collateral amount
    let collateral_amount = math::calculate_token_amount(
        amount,
        oracle_price,
        ctx.accounts.token_mint.decimals,
    )?;

    // Calculate fee
    let fee_amount = amount
        .checked_mul(ctx.accounts.stablecoin_mint.settings.fee_basis_points as u64)
        .and_then(|v| v.checked_div(10000))
        .ok_or(error!(StableFunError::MathOverflow))?;

    let burn_amount = amount
        .checked_add(fee_amount)
        .ok_or(error!(StableFunError::MathOverflow))?;

    // Calculate remaining amounts
    let remaining_collateral = ctx.accounts.vault
        .total_collateral
        .checked_sub(collateral_amount)
        .ok_or(error!(StableFunError::MathOverflow))?;

    let remaining_supply = ctx.accounts.stablecoin_mint
        .current_supply
        .checked_sub(burn_amount)
        .ok_or(error!(StableFunError::MathOverflow))?;

    // Validate collateral ratio if there's remaining supply
    if remaining_supply > 0 {
        ValidationService::validate_collateral_ratio(
            remaining_collateral,
            remaining_supply,
            ctx.accounts.stablecoin_mint.settings.min_collateral_ratio,
        )?;
    }

    // Burn stablecoins
    token::burn(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            token::Burn {
                mint: ctx.accounts.token_mint.to_account_info(),
                from: ctx.accounts.user_token_account.to_account_info(),
                authority: ctx.accounts.burn_authority.to_account_info(),
            },
            &[&[
                b"mint-authority",
                ctx.accounts.stablecoin_mint.key().as_ref(),
                &[ctx.bumps.burn_authority],
            ]],
        ),
        burn_amount,
    )?;

    // Transfer collateral back to user
    token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            token::Transfer {
                from: ctx.accounts.vault_stablebond_account.to_account_info(),
                to: ctx.accounts.user_stablebond_account.to_account_info(),
                authority: ctx.accounts.vault.to_account_info(),
            },
            &[&[
                b"vault",
                ctx.accounts.stablecoin_mint.key().as_ref(),
                &[ctx.bumps.vault],
            ]],
        ),
        collateral_amount,
    )?;

    // Update vault state
    ctx.accounts.vault.total_collateral = remaining_collateral;
    ctx.accounts.vault.total_value_locked = ctx.accounts.vault
        .total_value_locked
        .checked_sub(amount)
        .ok_or(error!(StableFunError::MathOverflow))?;
    
    ctx.accounts.vault.withdrawal_count = ctx.accounts.vault
        .withdrawal_count
        .checked_add(1)
        .ok_or(error!(StableFunError::MathOverflow))?;
    
    ctx.accounts.vault.last_withdrawal_time = Clock::get()?.unix_timestamp;

    // Update stablecoin state
    ctx.accounts.stablecoin_mint.current_supply = remaining_supply;
    ctx.accounts.stablecoin_mint.stats.total_burned = ctx.accounts.stablecoin_mint
        .stats
        .total_burned
        .checked_add(amount)
        .ok_or(error!(StableFunError::MathOverflow))?;
    
    ctx.accounts.stablecoin_mint.stats.total_fees = ctx.accounts.stablecoin_mint
        .stats
        .total_fees
        .checked_add(fee_amount)
        .ok_or(error!(StableFunError::MathOverflow))?;
    
    ctx.accounts.stablecoin_mint.last_updated = Clock::get()?.unix_timestamp;

    emit!(RedeemEvent {
        stablecoin_mint: ctx.accounts.stablecoin_mint.key(),
        user: ctx.accounts.user.key(),
        amount,
        fee_amount,
        collateral_amount,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

#[event]
pub struct RedeemEvent {
    pub stablecoin_mint: Pubkey,
    pub user: Pubkey,
    pub amount: u64,
    pub fee_amount: u64,
    pub collateral_amount: u64,
    pub timestamp: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fee_calculation() {
        let fee_basis_points = 30; // 0.3%
        let amount: u64 = 1_000_000;
        
        let fee = amount
            .checked_mul(fee_basis_points as u64)
            .and_then(|v| v.checked_div(10000))
            .unwrap();
            
        assert_eq!(fee, 3_000);
    }

    #[test]
    fn test_remaining_collateral_ratio() {
        let total_collateral = 1_500_000;
        let redeem_amount = 500_000;
        let min_ratio = 15000; // 150%
        
        let remaining_collateral = total_collateral - redeem_amount;
        let remaining_supply = 1_000_000;
        
        let ratio = (remaining_collateral as u128)
            .checked_mul(10000)
            .unwrap()
            .checked_div(remaining_supply as u128)
            .unwrap() as u16;
            
        assert!(ratio >= min_ratio);
    }
}