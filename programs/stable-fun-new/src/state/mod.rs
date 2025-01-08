use anchor_lang::prelude::*;

pub mod stablecoin;
pub mod vault;

pub use stablecoin::*;
pub use vault::*;

// Common constants shared across modules
pub const MAX_NAME_LENGTH: usize = 32;
pub const MAX_SYMBOL_LENGTH: usize = 10;
pub const MAX_CURRENCY_LENGTH: usize = 10;
pub const DISCRIMINATOR_LENGTH: usize = 8;
pub const PUBKEY_LENGTH: usize = 32;

// Price precision constants
pub const PRICE_DECIMALS: u8 = 6;
pub const PRICE_SCALE: u64 = 10_u64.pow(PRICE_DECIMALS as u32);

/// Common trait for state accounts
pub trait StateAccount {
    const LEN: usize;
}

// Price data from oracle - moved to common module since it's used across
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct PriceData {
    pub price: u64,
    pub last_updated: i64,
    pub confidence: u64,
}

impl PriceData {
    pub fn new(price: u64, last_updated: i64, confidence: u64) -> Self {
        Self {
            price,
            last_updated,
            confidence,
        }
    }

    pub fn is_valid(&self, max_age: i64, max_confidence: u64) -> bool {
        let now = Clock::get().unwrap().unix_timestamp;
        self.price > 0 
            && self.confidence <= max_confidence
            && (now - self.last_updated) <= max_age
    }
}