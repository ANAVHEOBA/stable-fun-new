use anchor_lang::prelude::*;
use super::{StateAccount, DISCRIMINATOR_LENGTH, PUBKEY_LENGTH};
use crate::error::StableFunError;

#[account]
#[derive(Debug)]
pub struct StablecoinVault {
    pub stablecoin_mint: Pubkey,
    pub authority: Pubkey,
    pub collateral_account: Pubkey,
    pub total_collateral: u64,
    pub total_value_locked: u64,
    pub current_ratio: u16,
    pub last_deposit_time: i64,
    pub last_withdrawal_time: i64,
    pub deposit_count: u32,
    pub withdrawal_count: u32,
    pub bump: u8,
}

impl StateAccount for StablecoinVault {
    const LEN: usize = DISCRIMINATOR_LENGTH +
        PUBKEY_LENGTH +    // stablecoin_mint
        PUBKEY_LENGTH +    // authority
        PUBKEY_LENGTH +    // collateral_account
        8 +               // total_collateral
        8 +               // total_value_locked
        2 +               // current_ratio
        8 +               // last_deposit_time
        8 +               // last_withdrawal_time
        4 +               // deposit_count
        4 +               // withdrawal_count
        1;               // bump
}

impl StablecoinVault {
    pub fn new(
        stablecoin_mint: Pubkey,
        authority: Pubkey,
        collateral_account: Pubkey,
        bump: u8,
    ) -> Self {
        Self {
            stablecoin_mint,
            authority,
            collateral_account,
            total_collateral: 0,
            total_value_locked: 0,
            current_ratio: 0,
            last_deposit_time: 0,
            last_withdrawal_time: 0,
            deposit_count: 0,
            withdrawal_count: 0,
            bump,
        }
    }

    pub fn process_deposit(
        &mut self,
        amount: u64,
        value: u64,
        clock: &Sysvar<Clock>,
    ) -> Result<()> {
        self.total_collateral = self.total_collateral
            .checked_add(amount)
            .ok_or(error!(StableFunError::MathOverflow))?;

        self.total_value_locked = self.total_value_locked
            .checked_add(value)
            .ok_or(error!(StableFunError::MathOverflow))?;

        self.last_deposit_time = clock.unix_timestamp;
        self.deposit_count = self.deposit_count
            .checked_add(1)
            .ok_or(error!(StableFunError::MathOverflow))?;

        self.update_collateral_ratio()?;
        Ok(())
    }

    pub fn process_withdrawal(
        &mut self,
        amount: u64,
        value: u64,
        clock: &Sysvar<Clock>,
    ) -> Result<()> {
        require!(
            amount <= self.total_collateral,
            StableFunError::InsufficientCollateral
        );

        self.total_collateral = self.total_collateral
            .checked_sub(amount)
            .ok_or(error!(StableFunError::MathOverflow))?;

        self.total_value_locked = self.total_value_locked
            .checked_sub(value)
            .ok_or(error!(StableFunError::MathOverflow))?;

        self.last_withdrawal_time = clock.unix_timestamp;
        self.withdrawal_count = self.withdrawal_count
            .checked_add(1)
            .ok_or(error!(StableFunError::MathOverflow))?;

        self.update_collateral_ratio()?;
        Ok(())
    }

    pub fn update_collateral_ratio(&mut self) -> Result<()> {
        if self.total_value_locked == 0 || self.total_collateral == 0 {
            self.current_ratio = 0;
            return Ok(());
        }

        let ratio = (self.total_value_locked as u128)
            .checked_mul(10000)
            .ok_or(error!(StableFunError::MathOverflow))?
            .checked_div(self.total_collateral as u128)
            .ok_or(error!(StableFunError::MathOverflow))?;

        self.current_ratio = u16::try_from(ratio)
            .map_err(|_| error!(StableFunError::MathOverflow))?;

        Ok(())
    }

    pub fn can_withdraw(&self, amount: u64, min_ratio: u16) -> bool {
        if amount >= self.total_collateral {
            return false;
        }

        let new_collateral = match self.total_collateral.checked_sub(amount) {
            Some(val) if val > 0 => val,
            _ => return false,
        };

        let new_ratio = match (self.total_value_locked as u128)
            .checked_mul(10000)
            .and_then(|v| v.checked_div(new_collateral as u128))
            .and_then(|r| u16::try_from(r).ok())
        {
            Some(ratio) => ratio,
            None => return false,
        };

        new_ratio >= min_ratio
    }

    pub fn get_vault_seeds<'a>(vault_bump: &'a u8) -> [&'a [u8]; 2] {
        [b"vault", std::slice::from_ref(vault_bump)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vault_creation() {
        let vault = StablecoinVault::new(
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            255,
        );

        assert_eq!(vault.total_collateral, 0);
        assert_eq!(vault.current_ratio, 0);
        assert_eq!(vault.deposit_count, 0);
    }

    #[test]
    fn test_collateral_ratio_calculation() {
        let mut vault = StablecoinVault::new(
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            255,
        );

        vault.total_collateral = 1000;
        vault.total_value_locked = 1500;

        assert!(vault.update_collateral_ratio().is_ok());
        assert_eq!(vault.current_ratio, 15000); // 150% = 15000 basis points
    }

    #[test]
    fn test_withdrawal_validation() {
        let mut vault = StablecoinVault::new(
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            255,
        );

        vault.total_collateral = 1000;
        vault.total_value_locked = 1500;
        vault.update_collateral_ratio().unwrap();

        assert!(vault.can_withdraw(100, 14000));  // Should allow withdrawal maintaining 140% ratio
        assert!(!vault.can_withdraw(900, 14000)); // Should prevent withdrawal below 140% ratio
    }
}