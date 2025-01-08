use anchor_lang::prelude::*;
use crate::error::StableFunError;
use crate::state::StateAccount; 

// Constants
pub const MAX_NAME_LENGTH: usize = 32;
pub const MAX_SYMBOL_LENGTH: usize = 10;
pub const MAX_CURRENCY_LENGTH: usize = 10;
pub const DISCRIMINATOR_LENGTH: usize = 8;
pub const PUBKEY_LENGTH: usize = 32;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, Default)]
pub struct StablecoinSettings {
    /// Fee in basis points (1/10000)
    pub fee_basis_points: u16,
    /// Maximum supply of stablecoins
    pub max_supply: u64,
    /// Minimum collateral ratio (e.g. 150%)
    pub min_collateral_ratio: u16,
    /// Whether minting is paused
    pub mint_paused: bool,
    /// Whether redeeming is paused
    pub redeem_paused: bool,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug, Default)]
pub struct StablecoinStats {
    /// Total amount of stablecoins minted
    pub total_minted: u64,
    /// Total amount of stablecoins burned
    pub total_burned: u64,
    /// Total fees collected
    pub total_fees: u64,
    /// Number of unique holders
    pub holder_count: u32,
    /// Reserved for future use
    pub reserved: [u8; 24],
}

#[account]
#[derive(Debug, Default)]
pub struct StablecoinMint {
    /// The authority who can update settings
    pub authority: Pubkey,
    
    /// Name of the stablecoin
    pub name: String,
    
    /// Symbol of the stablecoin (e.g., "USDX")
    pub symbol: String,
    
    /// Target fiat currency (e.g., "USD", "MXN")
    pub target_currency: String,
    
    /// The SPL token mint address
    pub token_mint: Pubkey,
    
    /// The stablebond token mint used as collateral
    pub stablebond_mint: Pubkey,
    
    /// The oracle feed for price data
    pub price_feed: Pubkey,
    
    /// Vault holding the collateral
    pub vault: Pubkey,
    
    /// Current supply of the stablecoin
    pub current_supply: u64,
    
    /// Configuration settings
    pub settings: StablecoinSettings,
    
    /// Statistics and metrics
    pub stats: StablecoinStats,
    
    /// Timestamp when the stablecoin was created
    pub created_at: i64,
    
    /// Last time settings were updated
    pub last_updated: i64,
}

impl StablecoinMint {
    pub const LEN: usize = DISCRIMINATOR_LENGTH +
        PUBKEY_LENGTH + // authority
        4 + MAX_NAME_LENGTH + // name (string)
        4 + MAX_SYMBOL_LENGTH + // symbol (string)
        4 + MAX_CURRENCY_LENGTH + // target_currency (string)
        PUBKEY_LENGTH + // token_mint
        PUBKEY_LENGTH + // stablebond_mint
        PUBKEY_LENGTH + // price_feed
        PUBKEY_LENGTH + // vault
        8 + // current_supply
        32 + // settings
        40 + // stats
        8 + // created_at
        8; // last_updated

    pub fn validate_name(name: &str) -> Result<()> {
        require!(
            !name.is_empty() && name.len() <= MAX_NAME_LENGTH,
            StableFunError::InvalidName
        );
        Ok(())
    }

    pub fn validate_symbol(symbol: &str) -> Result<()> {
        require!(
            !symbol.is_empty() && symbol.len() <= MAX_SYMBOL_LENGTH,
            StableFunError::InvalidSymbol
        );
        Ok(())
    }

    pub fn validate_currency(currency: &str) -> Result<()> {
        require!(
            !currency.is_empty() && currency.len() <= MAX_CURRENCY_LENGTH,
            StableFunError::InvalidCurrency
        );
        Ok(())
    }

    pub fn is_paused(&self) -> bool {
        self.settings.mint_paused || self.settings.redeem_paused
    }


    pub fn update_stats(&mut self, mint_amount: Option<u64>, burn_amount: Option<u64>, fees: Option<u64>) {
        if let Some(amount) = mint_amount {
            self.stats.total_minted = self.stats.total_minted.checked_add(amount).unwrap_or(self.stats.total_minted);
        }
        
        if let Some(amount) = burn_amount {
            self.stats.total_burned = self.stats.total_burned.checked_add(amount).unwrap_or(self.stats.total_burned);
        }
        
        if let Some(fee) = fees {
            self.stats.total_fees = self.stats.total_fees.checked_add(fee).unwrap_or(self.stats.total_fees);
        }
    }

    pub fn calculate_fee(&self, amount: u64) -> Result<u64> {
        amount
            .checked_mul(self.settings.fee_basis_points as u64)
            .and_then(|product| product.checked_div(10000))
            .ok_or(error!(StableFunError::MathOverflow))
    }


    pub fn is_mint_paused(&self) -> bool {
        self.settings.mint_paused
    }

    pub fn is_redeem_paused(&self) -> bool {
        self.settings.redeem_paused
    }

    pub fn can_mint(&self, amount: u64) -> bool {
        if self.is_mint_paused() {
            return false;
        }
        
        // Check against max supply
        self.current_supply
            .checked_add(amount)
            .map_or(false, |new_supply| new_supply <= self.settings.max_supply)
    }
}


impl StateAccount for StablecoinMint {
    const LEN: usize = StablecoinMint::LEN;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_name() {
        assert!(StablecoinMint::validate_name("USD Stablecoin").is_ok());
        assert!(StablecoinMint::validate_name("").is_err());
        assert!(StablecoinMint::validate_name(&"A".repeat(MAX_NAME_LENGTH + 1)).is_err());
    }

    #[test]
    fn test_validate_symbol() {
        assert!(StablecoinMint::validate_symbol("USDX").is_ok());
        assert!(StablecoinMint::validate_symbol("").is_err());
        assert!(StablecoinMint::validate_symbol(&"U".repeat(MAX_SYMBOL_LENGTH + 1)).is_err());
    }

    #[test]
    fn test_fee_calculation() {
        let mint = StablecoinMint {
            settings: StablecoinSettings {
                fee_basis_points: 30, // 0.3%
                ..Default::default()
            },
            ..Default::default()
        };

        assert_eq!(mint.calculate_fee(1000).unwrap(), 3); // 0.3% of 1000
        assert_eq!(mint.calculate_fee(10000).unwrap(), 30); // 0.3% of 10000
    }
}