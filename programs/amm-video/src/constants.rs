use anchor_lang::prelude::*;

#[constant]
pub const SEED: &str = "anchor";

pub const CONFIG_SEED: &[u8] = b"config";
pub const LP_SEED: &[u8] = b"lp";
pub const TREASURY_X_SEED: &[u8] = b"treasury_x";
pub const TREASURY_Y_SEED: &[u8] = b"treasury_y";

pub const MAX_FEE_BPS: u16 = 10_000;
pub const DEFAULT_FEE_BPS: u16 = 30;
pub const ZERO_FEE_BPS: u16 = 0;
pub const TOKEN_DECIMALS: u8 = 6;
pub const TOKEN_PRECISION: u32 = 1_000_000;
pub const DEFAULT_POOL_SEED: u64 = 123;
pub const ZERO_AMOUNT: u64 = 0;
