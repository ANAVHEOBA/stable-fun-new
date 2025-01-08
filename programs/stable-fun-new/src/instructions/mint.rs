use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount};
use switchboard_solana::{
    AggregatorAccountData,
    SWITCHBOARD_PROGRAM_ID,
};

use crate::state::{StablecoinMint, StablecoinVault};
use crate::error::*;
use super::{
    verify_oracle_price,
    calculate_token_amount,
    validate_collateral_ratio,
    transfer_tokens,
};

#[derive(Accounts)]
#[instruction(amount: u64)]
pub struct MintStablecoin<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        constraint = stablecoin_mint.authority == user.key() @ StablecoinError::UnauthorizedMint
    )]
    pub stablecoin_mint: Account<'info, StablecoinMint>,

    #[account(
        mut,
        constraint = vault.stablecoin_mint == stablecoin_mint.key() @ StablecoinError::InvalidVault
    )]
    pub vault: Account<'info, StablecoinVault>,

    #[account(
        mut,
        constraint = token_mint.key() == stablecoin_mint.token_mint @ StablecoinError::InvalidMint
    )]
    pub token_mint: Account<'info, Mint>,

    #[account(
        mut,
        constraint = user_token_account.mint == token_mint.key() @ StablecoinError::InvalidTokenAccount,
        constraint = user_token_account.owner == user.key() @ StablecoinError::InvalidTokenAccount
    )]
    pub user_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = user_stablebond_account.mint == stablecoin_mint.stablebond_mint @ StablecoinError::InvalidStablebond,
        constraint = user_stablebond_account.owner == user.key() @ StablecoinError::InvalidStablebond
    )]
    pub user_stablebond_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = vault_stablebond_account.key() == vault.collateral_account @ StablecoinError::InvalidVaultAccount
    )]
    pub vault_stablebond_account: Account<'info, TokenAccount>,

    /// The Switchboard V3 aggregator account
    #[account(
        constraint = price_feed.key() == stablecoin_mint.price_feed @ StablecoinError::InvalidOracle
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
    require!(!stablecoin_mint.settings.mint_paused, StablecoinError::MintPaused);

    // Validate amount
    require!(amount > 0, StablecoinError::InvalidAmount);
    require!(
        stablecoin_mint.current_supply.checked_add(amount).unwrap() <= stablecoin_mint.settings.max_supply,
        StablecoinError::MaxSupplyExceeded
    );

    // Get oracle price using v3 price feed
    let oracle_price = verify_oracle_price(&ctx.accounts.price_feed)?;

    // Calculate required collateral amount
    let collateral_amount = calculate_token_amount(
        amount,
        oracle_price,
        ctx.accounts.token_mint.decimals,
    )?;

    // Calculate fees
    let fee_amount = amount
        .checked_mul(stablecoin_mint.settings.fee_basis_points as u64)
        .and_then(|v| v.checked_div(10000))
        .ok_or(StablecoinError::MathOverflow)?;

    let total_amount = amount.checked_add(fee_amount).ok_or(StablecoinError::MathOverflow)?;

    // Transfer stablebonds to vault
    transfer_tokens(
        &ctx.accounts.user_stablebond_account,
        &ctx.accounts.vault_stablebond_account,
        &ctx.accounts.user,
        &ctx.accounts.token_program,
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
    vault.total_collateral = vault.total_collateral.checked_add(collateral_amount).unwrap();
    vault.total_value_locked = vault
        .total_value_locked
        .checked_add(amount)
        .ok_or(StablecoinError::MathOverflow)?;
    vault.deposit_count = vault.deposit_count.checked_add(1).unwrap();
    vault.last_deposit_time = Clock::get()?.unix_timestamp;
    vault.update_collateral_ratio()?;

    // Update stablecoin state
    stablecoin_mint.current_supply = stablecoin_mint
        .current_supply
        .checked_add(total_amount)
        .ok_or(StablecoinError::MathOverflow)?;
    stablecoin_mint.stats.total_minted = stablecoin_mint
        .stats
        .total_minted
        .checked_add(amount)
        .ok_or(StablecoinError::MathOverflow)?;
    stablecoin_mint.stats.total_fees = stablecoin_mint
        .stats
        .total_fees
        .checked_add(fee_amount)
        .ok_or(StablecoinError::MathOverflow)?;

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

#[error_code]
pub enum StablecoinError {
    #[msg("Unauthorized mint attempt")]
    UnauthorizedMint,
    #[msg("Invalid vault account")]
    InvalidVault,
    #[msg("Invalid mint account")]
    InvalidMint,
    #[msg("Invalid token account")]
    InvalidTokenAccount,
    #[msg("Invalid stablebond account")]
    InvalidStablebond,
    #[msg("Invalid vault token account")]
    InvalidVaultAccount,
    #[msg("Invalid oracle account")]
    InvalidOracle,
    #[msg("Invalid amount")]
    InvalidAmount,
    #[msg("Math overflow")]
    MathOverflow,
    #[msg("Mint is paused")]
    MintPaused,
    #[msg("Max supply exceeded")]
    MaxSupplyExceeded,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mint_validation() {
        // Add tests here
    }
}