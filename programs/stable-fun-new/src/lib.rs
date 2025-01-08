use anchor_lang::prelude::*;

declare_id!("8At1GLJvdVcY6LM7iPax9ShRAtAwKw41R87dd2MpnqHQ");

pub mod state;
pub mod instructions;
pub mod error;
pub mod utils;
pub mod constants;

use instructions::*;
use error::StableFunError;
use constants::{MIN_NAME_LENGTH, MIN_SYMBOL_LENGTH, MIN_COLLATERAL_RATIO};

#[program]
pub mod stable_fun_new {
    use super::*;

    #[inline(never)]
    pub fn initialize(
        ctx: Context<Initialize>,
        name: String,
        symbol: String,
        target_currency: String,
        initial_supply: u64,
    ) -> Result<()> {
        msg!("Initializing with name: {}, symbol: {}", name, symbol);
        require!(name.len() >= MIN_NAME_LENGTH, StableFunError::NameTooShort);
        require!(symbol.len() >= MIN_SYMBOL_LENGTH, StableFunError::SymbolTooShort);
        instructions::initialize::handler(ctx, name, symbol, target_currency, initial_supply)
    }

    #[inline(never)]
    pub fn mint(ctx: Context<MintStablecoin>, amount: u64) -> Result<()> {
        msg!("Minting {} tokens", amount);
        require!(amount > 0, StableFunError::InvalidAmount);
        instructions::mint::handler(ctx, amount)
    }

    #[inline(never)]
    pub fn redeem(ctx: Context<RedeemStablecoin>, amount: u64) -> Result<()> {
        msg!("Redeeming {} tokens", amount);
        require!(amount > 0, StableFunError::InvalidAmount);
        instructions::redeem::handler(ctx, amount)
    }

    #[inline(never)]
    pub fn update_settings(
        ctx: Context<UpdateSettings>,
        params: UpdateSettingsParams,
    ) -> Result<()> {
        msg!("Updating settings");
        require!(
            params.min_collateral_ratio.unwrap_or(MIN_COLLATERAL_RATIO) >= MIN_COLLATERAL_RATIO,
            StableFunError::CollateralRatioTooLow
        );
        instructions::update::handler(ctx, params)
    }
}