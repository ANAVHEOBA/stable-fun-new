// Seeds for PDAs
pub const STABLECOIN_SEED: &[u8] = b"stablecoin";
pub const VAULT_SEED: &[u8] = b"vault";
pub const MINT_AUTHORITY_SEED: &[u8] = b"mint-authority";

// Validation constants
pub const MIN_NAME_LENGTH: usize = 3;
pub const MIN_SYMBOL_LENGTH: usize = 2;
pub const MAX_NAME_LENGTH: usize = 32;
pub const MAX_SYMBOL_LENGTH: usize = 10;

// Financial constants
pub const BASIS_POINTS_DIVISOR: u16 = 10000;
pub const DEFAULT_COLLATERAL_RATIO: u16 = 15000; // 150%
pub const MIN_COLLATERAL_RATIO: u16 = 10000;     // 100%
pub const MAX_COLLATERAL_RATIO: u16 = 30000;     // 300%
pub const MAX_FEE_BPS: u16 = 1000;               // 10%

// Oracle constants
pub const PRICE_DECIMALS: u8 = 6;
pub const PRICE_SCALE: u64 = 10_u64.pow(PRICE_DECIMALS as u32);
pub const MAX_PRICE_AGE: i64 = 300;              // 5 minutes
pub const MAX_PRICE_CONFIDENCE: u64 = PRICE_SCALE / 100; // 1%

// Supply limits
pub const MIN_SUPPLY: u64 = 1_000;               // 1,000 units
pub const MAX_SUPPLY: u64 = 1_000_000_000;       // 1 billion units

// Time constants
pub const MIN_WITHDRAWAL_DELAY: i64 = 60;        // 1 minute
pub const MAX_WITHDRAWAL_DELAY: i64 = 86400;     // 24 hours