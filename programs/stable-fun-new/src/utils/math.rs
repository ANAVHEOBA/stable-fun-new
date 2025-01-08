use anchor_lang::prelude::*;
use crate::error::StableFunError;

pub fn checked_mul(a: u64, b: u64) -> Result<u64> {
    a.checked_mul(b).ok_or(error!(StableFunError::MathOverflow))
}

pub fn checked_div(a: u64, b: u64) -> Result<u64> {
    a.checked_div(b).ok_or(error!(StableFunError::MathOverflow))
}

pub fn calculate_token_amount(
    amount: u64,
    price: u64,
    decimals: u8,
) -> Result<u64> {
    amount
        .checked_mul(price)
        .and_then(|v| v.checked_div(10u64.pow(decimals as u32)))
        .ok_or(error!(StableFunError::MathOverflow))
}