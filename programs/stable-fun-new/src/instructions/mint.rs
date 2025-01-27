use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Mint};
use switchboard_solana::AggregatorAccountData;

use crate::state::{StablecoinMint, StablecoinVault};
use crate::error::StableFunError;
use crate::utils::oracle::OracleService;
use crate::utils::validation::ValidationService;
use crate::utils::math;

#[derive(Accounts)]
#[instruction(amount: u64)]
pub struct MintStablecoin<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        constraint = stablecoin_mint.authority == user.key() @ StableFunError::UnauthorizedMint
    )]
    pub stablecoin_mint: Account<'info, StablecoinMint>,

    #[account(
        mut,
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

    /// CHECK: PDA used as mint authority
    #[account(
        seeds = [b"mint-authority", stablecoin_mint.key().as_ref()],
        bump
    )]
    pub mint_authority: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<MintStablecoin>, amount: u64) -> Result<()> {
    let stablecoin_mint = &mut ctx.accounts.stablecoin_mint;
    let vault = &mut ctx.accounts.vault;

    // Validate mint is not paused
    require!(!stablecoin_mint.settings.mint_paused, StableFunError::MintingPaused);

    // Validate amount
    require!(amount > 0, StableFunError::InvalidAmount);
    require!(
        stablecoin_mint.current_supply.checked_add(amount).unwrap() <= stablecoin_mint.settings.max_supply,
        StableFunError::MaxSupplyExceeded
    );

    // Get oracle price
    let oracle_price = OracleService::verify_oracle_price(&ctx.accounts.price_feed)?;

    // Calculate required collateral amount
    let collateral_amount = math::calculate_token_amount(
        amount,
        oracle_price,
        ctx.accounts.token_mint.decimals,
    )?;

    // Calculate fees
    let fee_amount = amount
        .checked_mul(stablecoin_mint.settings.fee_basis_points as u64)
        .and_then(|v| v.checked_div(10000))
        .ok_or(error!(StableFunError::MathOverflow))?;

    let total_amount = amount
        .checked_add(fee_amount)
        .ok_or(error!(StableFunError::MathOverflow))?;

    // Transfer stablebonds to vault
    token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            token::Transfer {
                from: ctx.accounts.user_stablebond_account.to_account_info(),
                to: ctx.accounts.vault_stablebond_account.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            },
        ),
        collateral_amount,
    )?;

    // Mint stablecoins to user
    token::mint_to(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            token::MintTo {
                mint: ctx.accounts.token_mint.to_account_info(),
                to: ctx.accounts.user_token_account.to_account_info(),
                authority: ctx.accounts.mint_authority.to_account_info(),
            },
            &[&[
                b"mint-authority",
                stablecoin_mint.key().as_ref(),
                &[ctx.bumps.mint_authority],
            ]],
        ),
        total_amount,
    )?;

    // Update vault state
    vault.total_collateral = vault
        .total_collateral
        .checked_add(collateral_amount)
        .ok_or(error!(StableFunError::MathOverflow))?;
    
    vault.total_value_locked = vault
        .total_value_locked
        .checked_add(amount)
        .ok_or(error!(StableFunError::MathOverflow))?;
    
    vault.deposit_count = vault
        .deposit_count
        .checked_add(1)
        .ok_or(error!(StableFunError::MathOverflow))?;
    
    vault.last_deposit_time = Clock::get()?.unix_timestamp;
    
    // Update collateral ratio
    ValidationService::update_collateral_ratio(vault)?;

    // Update stablecoin state
    stablecoin_mint.current_supply = stablecoin_mint
        .current_supply
        .checked_add(total_amount)
        .ok_or(error!(StableFunError::MathOverflow))?;
    
    stablecoin_mint.stats.total_minted = stablecoin_mint
        .stats
        .total_minted
        .checked_add(amount)
        .ok_or(error!(StableFunError::MathOverflow))?;
    
    stablecoin_mint.stats.total_fees = stablecoin_mint
        .stats
        .total_fees
        .checked_add(fee_amount)
        .ok_or(error!(StableFunError::MathOverflow))?;

    stablecoin_mint.last_updated = Clock::get()?.unix_timestamp;

    emit!(MintEvent {
        stablecoin_mint: stablecoin_mint.key(),
        user: ctx.accounts.user.key(),
        amount,
        fee_amount,
        collateral_amount,
        timestamp: Clock::get()?.unix_timestamp,
    });

    Ok(())
}

#[event]
pub struct MintEvent {
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
    use anchor_lang::solana_program::system_program;

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
    fn test_total_amount_calculation() {
        let amount: u64 = 1_000_000;
        let fee = 3_000;
        
        let total = amount.checked_add(fee).unwrap();
        assert_eq!(total, 1_003_000);
    }
}