use anchor_lang::prelude::*;

#[error_code]
pub enum StableFunError {
    #[msg("Name must be at least 3 characters")]
    NameTooShort,
    
    #[msg("Symbol must be at least 2 characters")]
    SymbolTooShort,
    
    #[msg("Invalid name provided")]
    InvalidName,
    
    #[msg("Invalid symbol provided")]
    InvalidSymbol,
    
    #[msg("Invalid currency specified")]
    InvalidCurrency,
    
    #[msg("Invalid amount")]
    InvalidAmount,
    
    #[msg("Insufficient collateral")]
    InsufficientCollateral,
    
    #[msg("Invalid oracle price")]
    InvalidOraclePrice,
    
    #[msg("Stale oracle price")]
    StaleOraclePrice,
    
    #[msg("Math overflow in calculation")]
    MathOverflow,
    
    #[msg("Invalid token account")]
    InvalidTokenAccount,
    
    #[msg("Invalid vault account")]
    InvalidVault,
    
    #[msg("Maximum supply exceeded")]
    MaxSupplyExceeded,
    
    #[msg("Insufficient balance")]
    InsufficientBalance,
    
    #[msg("Account owner mismatch")]
    AccountOwnerMismatch,
    
    #[msg("Invalid oracle account")]
    InvalidOracle,
    
    #[msg("Invalid mint account")]
    InvalidMint,
    
    #[msg("Invalid stablebond account")]
    InvalidStablebond,
    
    #[msg("Invalid vault token account")]
    InvalidVaultAccount,
    
    #[msg("Minting is paused")]
    MintingPaused,
    
    #[msg("Redeeming is paused")]
    RedeemingPaused,
    
    #[msg("Collateral ratio too low")]
    CollateralRatioTooLow,
    
    #[msg("Collateral ratio too high")]
    CollateralRatioTooHigh,
    
    #[msg("Fee too high")]
    FeeTooHigh,
    
    #[msg("Amount too small")]
    AmountTooSmall,
    
    #[msg("Amount too large")]
    AmountTooLarge,
    
    #[msg("Invalid token owner")]
    InvalidTokenOwner,
    
    #[msg("Empty vault")]
    EmptyVault,

    #[msg("Invalid max supply")]
    InvalidMaxSupply,
}

// Helper functions for common error checks
impl StableFunError {
    pub fn check_name_length(name: &str) -> Result<()> {
        require!(
            name.len() >= 3,
            StableFunError::NameTooShort
        );
        Ok(())
    }

    pub fn check_symbol_length(symbol: &str) -> Result<()> {
        require!(
            symbol.len() >= 2,
            StableFunError::SymbolTooShort
        );
        Ok(())
    }

    pub fn check_amount(amount: u64, min: u64, max: u64) -> Result<()> {
        require!(
            amount >= min,
            StableFunError::AmountTooSmall
        );
        require!(
            amount <= max,
            StableFunError::AmountTooLarge
        );
        Ok(())
    }

    pub fn check_collateral_ratio(ratio: u16, min: u16, max: u16) -> Result<()> {
        require!(
            ratio >= min,
            StableFunError::CollateralRatioTooLow
        );
        require!(
            ratio <= max,
            StableFunError::CollateralRatioTooHigh
        );
        Ok(())
    }

    pub fn check_token_owner(owner: &Pubkey, expected: &Pubkey) -> Result<()> {
        require!(
            owner == expected,
            StableFunError::InvalidTokenOwner
        );
        Ok(())
    }

    pub fn check_vault_balance(balance: u64) -> Result<()> {
        require!(
            balance > 0,
            StableFunError::EmptyVault
        );
        Ok(())
    }

    pub fn check_oracle_price(price: u64, max_staleness: i64, now: i64) -> Result<()> {
        require!(
            price > 0,
            StableFunError::InvalidOraclePrice
        );
        require!(
            now - max_staleness <= now,
            StableFunError::StaleOraclePrice
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_name_length_validation() {
        assert!(StableFunError::check_name_length("ABC").is_ok());
        assert!(StableFunError::check_name_length("AB").is_err());
    }

    #[test]
    fn test_symbol_length_validation() {
        assert!(StableFunError::check_symbol_length("ABC").is_ok());
        assert!(StableFunError::check_symbol_length("A").is_err());
    }

    #[test]
    fn test_amount_validation() {
        assert!(StableFunError::check_amount(500, 100, 1000).is_ok());
        assert!(StableFunError::check_amount(50, 100, 1000).is_err());
        assert!(StableFunError::check_amount(1500, 100, 1000).is_err());
    }

    #[test]
    fn test_collateral_ratio_validation() {
        assert!(StableFunError::check_collateral_ratio(150, 100, 200).is_ok());
        assert!(StableFunError::check_collateral_ratio(90, 100, 200).is_err());
        assert!(StableFunError::check_collateral_ratio(250, 100, 200).is_err());
    }
}