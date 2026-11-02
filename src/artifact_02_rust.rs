//! Path: /src/engine/price_comparator_engine.rs
//! High-precision, zero-allocation math engine for cross-DEX Solana price comparisons.

use std::convert::TryInto;

/// Scale factor for fixed-point math: 10^12 precision
pub const PRECISION_SCALE: u128 = 1_000_000_000_000;
pub const Q64: u128 = 1 << 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PoolType {
    ConcentratedLiquidity, // Orca Whirlpools / Raydium CLMM
    ConstantProduct,       // Raydium v4 / Standard AMM
}

#[derive(Debug, Clone)]
pub struct PoolStateSnapshot {
    pub pool_address: [u8; 32],
    pub pool_type: PoolType,
    pub token_a_decimals: u8,
    pub token_b_decimals: u8,
    // For CLMM: sqrt_price in Q64.64 format
    pub sqrt_price_x64: u128,
    pub tick_current: i32,
    // For Constant Product: vault reserves
    pub reserve_a: u64,
    pub reserve_b: u64,
    pub fee_bps: u16,
}

#[derive(Debug, Clone)]
pub struct PythOracleSnapshot {
    pub price: i64,
    pub conf: u64,
    pub expo: i32,
    pub publish_time: i64,
}

#[derive(Debug, Clone)]
pub struct PriceComparisonResult {
    pub effective_price_a_in_b_scaled: u128,
    pub pyth_price_scaled: u128,
    pub absolute_divergence_bps: u64,
    pub is_within_oracle_confidence: bool,
    pub pool_type: PoolType,
}

#[derive(Debug, thiserror::Error)]
pub enum ComparatorError {
    #[error("Zero reserves in constant product pool")]
    ZeroReserves,
    #[error("Zero sqrt price in concentrated pool")]
    ZeroSqrtPrice,
    #[error("Math overflow occurred")]
    MathOverflow,
    #[error("Oracle price negative or invalid")]
    InvalidOraclePrice,
}

/// Computes the human-readable price of Token A in terms of Token B scaled to 1e12
pub fn calculate_effective_price(
    pool: &PoolStateSnapshot,
) -> Result<u128, ComparatorError> {
    match pool.pool_type {
        PoolType::ConstantProduct => {
            if pool.reserve_a == 0 || pool.reserve_b == 0 {
                return Err(ComparatorError::ZeroReserves);
            }

            // Normalizing decimals: price = (reserve_b / 10^b_dec) / (reserve_a / 10^a_dec)
            // price_scaled = (reserve_b * 10^a_dec * PRECISION_SCALE) / (reserve_a * 10^b_dec)
            let factor_a = 10u128
                .checked_pow(pool.token_a_decimals as u32)
                .ok_or(ComparatorError::MathOverflow)?;
            let factor_b = 10u128
                .checked_pow(pool.token_b_decimals as u32)
                .ok_or(ComparatorError::MathOverflow)?;

            let numerator = (pool.reserve_b as u128)
                .checked_mul(factor_a)
                .ok_or(ComparatorError::MathOverflow)?
                .checked_mul(PRECISION_SCALE)
                .ok_or(ComparatorError::MathOverflow)?;

            let denominator = (pool.reserve_a as u128)
                .checked_mul(factor_b)
                .ok_or(ComparatorError::MathOverflow)?;

            numerator
                .checked_div(denominator)
                .ok_or(ComparatorError::MathOverflow)
        }
        PoolType::ConcentratedLiquidity => {
            if pool.sqrt_price_x64 == 0 {
                return Err(ComparatorError::ZeroSqrtPrice);
            }

            // Price = (sqrt_price / 2^64)^2
            // P_unscaled = (sqrt_price_x64^2) / 2^128
            // Price normalized = P_unscaled * 10^(token_a_decimals - token_b_decimals)
            let sqrt_price = pool.sqrt_price_x64;
            
            // Perform division and multiplication keeping high precision
            // Let ratio = sqrt_price / 2^64
            // Scaled squared computation using 256-bit safe representation
            let sqrt_price_sq = (sqrt_price as u128)
                .checked_mul(sqrt_price)
                .ok_or(ComparatorError::MathOverflow)?;

            // Convert Q128.128 to fixed precision:
            // price_scaled = (sqrt_price_sq * PRECISION_SCALE) >> 128
            let raw_price_scaled = (sqrt_price_sq >> 64)
                .checked_mul(PRECISION_SCALE)
                .ok_or(ComparatorError::MathOverflow)?
                >> 64;

            // Adjust for decimals
            let a_dec = pool.token_a_decimals as i32;
            let b_dec = pool.token_b_decimals as i32;
            let dec_diff = a_dec - b_dec;

            let final_price_scaled = if dec_diff >= 0 {
                let multiplier = 10u128
                    .checked_pow(dec_diff as u32)
                    .ok_or(ComparatorError::MathOverflow)?;
                raw_price_scaled
                    .checked_mul(multiplier)
                    .ok_or(ComparatorError::MathOverflow)?
            } else {
                let divisor = 10u128
                    .checked_pow((-dec_diff) as u32)
                    .ok_or(ComparatorError::MathOverflow)?;
                raw_price_scaled
                    .checked_div(divisor)
                    .ok_or(ComparatorError::MathOverflow)?
            };

            Ok(final_price_scaled)
        }
    }
}

/// Normalizes Pyth oracle price to PRECISION_SCALE (1e12)
pub fn normalize_pyth_price(
    oracle: &PythOracleSnapshot,
) -> Result<(u128, u128), ComparatorError> {
    if oracle.price <= 0 {
        return Err(ComparatorError::InvalidOraclePrice);
    }

    let raw_price = oracle.price as u128;
    let raw_conf = oracle.conf as u128;

    let target_expo: i32 = 12; // Corresponding to 1e12
    let net_expo = target_expo + oracle.expo;

    let (scaled_price, scaled_conf) = if net_expo >= 0 {
        let mult = 10u128
            .checked_pow(net_expo as u32)
            .ok_or(ComparatorError::MathOverflow)?;
        (
            raw_price.checked_mul(mult).ok_or(ComparatorError::MathOverflow)?,
            raw_conf.checked_mul(mult).ok_or(ComparatorError::MathOverflow)?,
        )
    } else {
        let div = 10u128
            .checked_pow((-net_expo) as u32)
            .ok_or(ComparatorError::MathOverflow)?;
        (
            raw_price.checked_div(div).ok_or(ComparatorError::MathOverflow)?,
            raw_conf.checked_div(div).ok_or(ComparatorError::MathOverflow)?,
        )
    };

    Ok((scaled_price, scaled_conf))
}

/// Compares a single pool against the Pyth Oracle snapshot
pub fn compare_pool_vs_oracle(
    pool: &PoolStateSnapshot,
    oracle: &PythOracleSnapshot,
) -> Result<PriceComparisonResult, ComparatorError> {
    let pool_price = calculate_effective_price(pool)?;
    let (oracle_price, oracle_conf) = normalize_pyth_price(oracle)?;

    // Absolute difference
    let diff = if pool_price >= oracle_price {
        pool_price - oracle_price
    } else {
        oracle_price - pool_price
    };

    // Basis points divergence = (diff / oracle_price) * 10,000
    let divergence_bps = ((diff as f64 / oracle_price as f64) * 10_000.0) as u64;

    // Check if difference falls strictly within Pyth confidence interval
    let is_within_conf = diff <= oracle_conf;

    Ok(PriceComparisonResult {
        effective_price_a_in_b_scaled: pool_price,
        pyth_price_scaled: oracle_price,
        absolute_divergence_bps: divergence_bps,
        is_within_oracle_confidence: is_within_conf,
        pool_type: pool.pool_type.clone(),
    })
}