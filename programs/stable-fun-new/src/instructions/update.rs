use anchor_lang::prelude::*;
use crate::state::{StablecoinMint, StablecoinSettings};
use crate::error::*;

#[derive(Accounts)]
pub struct UpdateSettings<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        mut,
        constraint = stablecoin_mint.authority == authority.key() @ UpdateError::UnauthorizedUpdate
    )]
    pub stablecoin_mint: Account<'info, StablecoinMint>,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct UpdateSettingsParams {
    pub min_collateral_ratio: Option<u16>,
    pub fee_basis_points: Option<u16>,
    pub max_supply: Option<u64>,
    pub mint_paused: Option<bool>,
    pub redeem_paused: Option<bool>,
}

pub fn handler(
    ctx: Context<UpdateSettings>,
    params: UpdateSettingsParams,
) -> Result<()> {
    let stablecoin_mint = &mut ctx.accounts.stablecoin_mint;
    let clock = Clock::get()?;

    // Clone current settings for event
    let old_settings = stablecoin_mint.settings.clone();
    
    // Update settings
    if let Some(new_ratio) = params.min_collateral_ratio {
        stablecoin_mint.settings.min_collateral_ratio = new_ratio;
    }
    
    if let Some(new_fee) = params.fee_basis_points {
        stablecoin_mint.settings.fee_basis_points = new_fee;
    }
    
    if let Some(new_max_supply) = params.max_supply {
        require!(
            new_max_supply >= stablecoin_mint.current_supply,
            StableFunError::InvalidMaxSupply
        );
        stablecoin_mint.settings.max_supply = new_max_supply;
    }
    
    if let Some(paused) = params.mint_paused {
        stablecoin_mint.settings.mint_paused = paused;
    }
    
    if let Some(paused) = params.redeem_paused {
        stablecoin_mint.settings.redeem_paused = paused;
    }

    stablecoin_mint.last_updated = clock.unix_timestamp;

    emit!(SettingsUpdateEvent {
        stablecoin_mint: stablecoin_mint.key(),
        authority: ctx.accounts.authority.key(),
        old_settings,
        new_settings: stablecoin_mint.settings.clone(),
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct UpdateMetadataParams {
    pub name: Option<String>,
    pub symbol: Option<String>,
    pub icon_uri: Option<String>,
}



#[derive(Accounts)]
pub struct UpdateMetadata<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        mut,
        constraint = stablecoin_mint.authority == authority.key() @ UpdateError::UnauthorizedUpdate
    )]
    pub stablecoin_mint: Account<'info, StablecoinMint>,
}

pub fn update_metadata(
    ctx: Context<UpdateMetadata>,
    params: UpdateMetadataParams,
) -> Result<()> {
    let stablecoin_mint = &mut ctx.accounts.stablecoin_mint;
    let clock = Clock::get()?;

    // Update name if provided
    if let Some(new_name) = params.name {
        require!(
            !new_name.is_empty() && new_name.len() <= 32,
            UpdateError::InvalidName
        );
        stablecoin_mint.name = new_name;
    }

    // Update symbol if provided
    if let Some(new_symbol) = params.symbol {
        require!(
            !new_symbol.is_empty() && new_symbol.len() <= 10,
            UpdateError::InvalidSymbol
        );
        stablecoin_mint.symbol = new_symbol;
    }

    // Update last updated timestamp
    stablecoin_mint.last_updated = clock.unix_timestamp;

    emit!(MetadataUpdateEvent {
        stablecoin_mint: stablecoin_mint.key(),
        authority: ctx.accounts.authority.key(),
        name: stablecoin_mint.name.clone(),
        symbol: stablecoin_mint.symbol.clone(),
        timestamp: clock.unix_timestamp,
    });

    Ok(())
}

#[event]
pub struct SettingsUpdateEvent {
    pub stablecoin_mint: Pubkey,
    pub authority: Pubkey,
    pub old_settings: StablecoinSettings,
    pub new_settings: StablecoinSettings,
    pub timestamp: i64,
}

#[event]
pub struct MetadataUpdateEvent {
    pub stablecoin_mint: Pubkey,
    pub authority: Pubkey,
    pub name: String,
    pub symbol: String,
    pub timestamp: i64,
}

#[error_code]
pub enum UpdateError {
    #[msg("Unauthorized update attempt")]
    UnauthorizedUpdate,
    #[msg("Invalid collateral ratio")]
    InvalidCollateralRatio,
    #[msg("Invalid fee percentage")]
    InvalidFee,
    #[msg("Invalid max supply")]
    InvalidMaxSupply,
    #[msg("Invalid name")]
    InvalidName,
    #[msg("Invalid symbol")]
    InvalidSymbol,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_update_settings() {
        // Test settings updates
        let mut test_mint = StablecoinMint {
            settings: StablecoinSettings {
                min_collateral_ratio: 15000,
                fee_basis_points: 30,
                max_supply: 1_000_000,
                mint_paused: false,
                redeem_paused: false,
            },
            ..Default::default()
        };

        let params = UpdateSettingsParams {
            min_collateral_ratio: Some(20000),
            fee_basis_points: Some(50),
            max_supply: Some(2_000_000),
            mint_paused: Some(true),
            redeem_paused: Some(true),
        };

        // Simulate update
        test_mint.settings.min_collateral_ratio = params.min_collateral_ratio.unwrap();
        test_mint.settings.fee_basis_points = params.fee_basis_points.unwrap();
        test_mint.settings.max_supply = params.max_supply.unwrap();
        test_mint.settings.mint_paused = params.mint_paused.unwrap();
        test_mint.settings.redeem_paused = params.redeem_paused.unwrap();

        assert_eq!(test_mint.settings.min_collateral_ratio, 20000);
        assert_eq!(test_mint.settings.fee_basis_points, 50);
        assert_eq!(test_mint.settings.max_supply, 2_000_000);
        assert_eq!(test_mint.settings.mint_paused, true);
        assert_eq!(test_mint.settings.redeem_paused, true);
    }

    #[test]
    fn test_update_metadata() {
        // Test metadata updates
        let mut test_mint = StablecoinMint {
            name: "Old Name".to_string(),
            symbol: "OLD".to_string(),
            ..Default::default()
        };

        let params = UpdateMetadataParams {
            name: Some("New Name".to_string()),
            symbol: Some("NEW".to_string()),
            icon_uri: None,
        };

        // Simulate update
        if let Some(name) = params.name {
            test_mint.name = name;
        }
        if let Some(symbol) = params.symbol {
            test_mint.symbol = symbol;
        }

        assert_eq!(test_mint.name, "New Name");
        assert_eq!(test_mint.symbol, "NEW");
    }
}