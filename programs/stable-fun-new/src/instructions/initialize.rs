use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};
use switchboard_solana::AggregatorAccountData;

use crate::state::{StablecoinMint, StablecoinVault, StateAccount};
use crate::state::stablecoin::{StablecoinSettings, StablecoinStats};
use crate::error::StableFunError;

// Constants
pub const STABLECOIN_SEED: &[u8] = b"stablecoin";
pub const VAULT_SEED: &[u8] = b"vault";
pub const MINT_AUTHORITY_SEED: &[u8] = b"mint-authority";
pub const MIN_NAME_LENGTH: usize = 3;
pub const MIN_SYMBOL_LENGTH: usize = 2;
pub const DEFAULT_COLLATERAL_RATIO: u16 = 15000; // 150%

#[derive(Accounts)]
#[instruction(
    name: String,
    symbol: String,
    target_currency: String,
    initial_supply: u64
)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        init,
        payer = authority,
        space = StablecoinMint::LEN,
        seeds = [
            STABLECOIN_SEED,
            authority.key().as_ref(),
            symbol.as_bytes()
        ],
        bump
    )]
    pub stablecoin_mint: Account<'info, StablecoinMint>,

    #[account(
        init,
        payer = authority,
        mint::decimals = 6,
        mint::authority = mint_authority.key(),
    )]
    pub token_mint: Account<'info, Mint>,

    #[account(
        seeds = [
            MINT_AUTHORITY_SEED,
            stablecoin_mint.key().as_ref()
        ],
        bump
    )]
    /// CHECK: PDA used as mint authority
    pub mint_authority: UncheckedAccount<'info>,

    pub stablebond_mint: Account<'info, Mint>,

    #[account(
        init,
        payer = authority,
        space = StablecoinVault::LEN,
        seeds = [
            VAULT_SEED,
            stablecoin_mint.key().as_ref()
        ],
        bump
    )]
    pub vault: Account<'info, StablecoinVault>,

    #[account(
        init,
        payer = authority,
        token::mint = stablebond_mint,
        token::authority = vault,
    )]
    pub vault_token_account: Account<'info, TokenAccount>,

    /// Switchboard V3 aggregator account
    #[account(
        constraint = 
            price_feed.load()?.get_result().is_ok() @ StableFunError::InvalidOracle
    )]
    pub price_feed: AccountLoader<'info, AggregatorAccountData>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

pub fn handler(
    ctx: Context<Initialize>,
    name: String,
    symbol: String,
    target_currency: String,
    initial_supply: u64,
) -> Result<()> {
    // Validate inputs
    require!(
        name.len() >= MIN_NAME_LENGTH,
        StableFunError::NameTooShort
    );
    require!(
        symbol.len() >= MIN_SYMBOL_LENGTH,
        StableFunError::SymbolTooShort
    );
    require!(
        !target_currency.is_empty(),
        StableFunError::InvalidCurrency
    );

    // Verify oracle with V3 validation
    let oracle = ctx.accounts.price_feed.load()?;
    let result = oracle.get_result()
        .map_err(|_| error!(StableFunError::InvalidOraclePrice))?;

    require!(
        result.mantissa > 0,
        StableFunError::InvalidOraclePrice
    );

    let clock = Clock::get()?;
    
    // Initialize stablecoin mint account
    let stablecoin_mint = &mut ctx.accounts.stablecoin_mint;
    stablecoin_mint.authority = ctx.accounts.authority.key();
    stablecoin_mint.name = name.clone();
    stablecoin_mint.symbol = symbol.clone();
    stablecoin_mint.target_currency = target_currency.clone();
    stablecoin_mint.token_mint = ctx.accounts.token_mint.key();
    stablecoin_mint.stablebond_mint = ctx.accounts.stablebond_mint.key();
    stablecoin_mint.price_feed = ctx.accounts.price_feed.key();
    stablecoin_mint.vault = ctx.accounts.vault.key();
    stablecoin_mint.current_supply = 0;
    stablecoin_mint.created_at = clock.unix_timestamp;
    stablecoin_mint.last_updated = clock.unix_timestamp;

    // Initialize settings with default values
    stablecoin_mint.settings = StablecoinSettings {
        min_collateral_ratio: DEFAULT_COLLATERAL_RATIO,
        fee_basis_points: 30, // 0.3% fee
        max_supply: u64::MAX,
        mint_paused: false,
        redeem_paused: false,
    };

    // Initialize statistics
    stablecoin_mint.stats = StablecoinStats::default();

    // Initialize vault
    let vault = &mut ctx.accounts.vault;
    vault.stablecoin_mint = stablecoin_mint.key();
    vault.authority = ctx.accounts.authority.key();
    vault.collateral_account = ctx.accounts.vault_token_account.key();
    vault.total_collateral = 0;
    vault.total_value_locked = 0;
    vault.current_ratio = 0;
    vault.last_deposit_time = clock.unix_timestamp;
    vault.last_withdrawal_time = clock.unix_timestamp;
    vault.deposit_count = 0;
    vault.withdrawal_count = 0;
    vault.bump = ctx.bumps.vault;

    emit!(StablecoinInitialized {
        stablecoin_mint: stablecoin_mint.key(),
        authority: ctx.accounts.authority.key(),
        name,
        symbol,
        target_currency,
    });

    Ok(())
}

#[event]
#[derive(Clone, Debug)]
pub struct StablecoinInitialized {
    pub stablecoin_mint: Pubkey,
    pub authority: Pubkey,
    pub name: String,
    pub symbol: String,
    pub target_currency: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use anchor_lang::solana_program::system_program;

    #[test]
    fn test_validate_inputs() {
        // Test valid inputs
        let name = "Test Coin".to_string();
        let symbol = "TEST".to_string();
        let currency = "USD".to_string();

        assert!(name.len() >= MIN_NAME_LENGTH);
        assert!(symbol.len() >= MIN_SYMBOL_LENGTH);
        assert!(!currency.is_empty());

        // Test invalid inputs
        let short_name = "Te".to_string();
        let short_symbol = "T".to_string();
        let empty_currency = "".to_string();

        assert!(short_name.len() < MIN_NAME_LENGTH);
        assert!(short_symbol.len() < MIN_SYMBOL_LENGTH);
        assert!(empty_currency.is_empty());
    }
}