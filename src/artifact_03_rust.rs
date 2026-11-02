use anchor_lang::prelude::*;
use pyth_sdk_solana::load_price_feed_from_account_info;

declare_id!("PMatr1x9wTsmKz7vFeqU9KkY2h8u7QYgSvh6VnEwM5s");

pub const BPS_DENOMINATOR: u64 = 10_000;
pub const MAX_ALLOWED_STALENESS_SLOTS: u64 = 50; // ~20 seconds on Solana

#[program]
pub mod solana_price_matrix {
    use super::*;

    /// Initialize configuration for price guard policy
    pub fn initialize_matrix_config(
        ctx: Context<InitializeMatrixConfig>,
        max_allowed_divergence_bps: u16,
        max_staleness_slots: u64,
    ) -> Result<()> {
        require!(
            max_allowed_divergence_bps <= 1_000,
            PriceMatrixError::InvalidDivergenceBpsThreshold
        );
        require!(
            max_staleness_slots > 0 && max_staleness_slots <= 300,
            PriceMatrixError::InvalidStalenessThreshold
        );

        let config = &mut ctx.accounts.matrix_config;
        config.admin = ctx.accounts.admin.key();
        config.max_allowed_divergence_bps = max_allowed_divergence_bps;
        config.max_staleness_slots = max_staleness_slots;
        config.bump = ctx.bumps.matrix_config;

        msg!(
            "Price Matrix Config Initialized: Max BPS={}, Max Staleness Slots={}",
            max_allowed_divergence_bps,
            max_staleness_slots
        );
        Ok(())
    }

    /// Compare Pyth V2 price feed against an Orca-style CLMM Whirlpool sqrt-price
    pub fn compare_and_guard_price(
        ctx: Context<CompareAndGuardPrice>,
        expected_base_decimals: u8,
        expected_quote_decimals: u8,
    ) -> Result<()> {
        let clock = Clock::get()?;
        let current_slot = clock.slot;

        // 1. Validate Pyth Price Feed
        let pyth_price_info = &ctx.accounts.pyth_price_account;
        let price_feed = load_price_feed_from_account_info(pyth_price_info)
            .map_err(|_| PriceMatrixError::FailedToLoadPythFeed)?;

        let pyth_price_data = price_feed.get_price_unchecked();

        // Staleness guard check against current slot
        let slots_elapsed = current_slot.saturating_sub(pyth_price_data.publish_time as u64);
        require!(
            slots_elapsed <= ctx.accounts.matrix_config.max_staleness_slots,
            PriceMatrixError::PythPriceStale
        );

        // Discard negative or zero prices
        require!(
            pyth_price_data.price > 0,
            PriceMatrixError::InvalidOraclePrice
        );

        // Normalized Pyth Price (fixed to 6 decimal precision)
        let normalized_pyth_price_u64 = normalize_to_scale(
            pyth_price_data.price as u64,
            pyth_price_data.expo,
            6,
        )?;

        // 2. Validate and Compute Orca Whirlpool Price from SqrtPrice (Q64.64)
        let whirlpool_data = ctx.accounts.whirlpool_account.data.borrow();
        require!(
            whirlpool_data.len() >= 137, // Sqrt price offset is at index 65..81 in Orca Whirlpool
            PriceMatrixError::InvalidWhirlpoolAccountLayout
        );

        // Extract sqrt_price: u128 located at offset [65..81]
        let mut sqrt_price_bytes = [0u8; 16];
        sqrt_price_bytes.copy_from_slice(&whirlpool_data[65..81]);
        let sqrt_price_q64 = u128::from_le_bytes(sqrt_price_bytes);

        require!(sqrt_price_q64 > 0, PriceMatrixError::InvalidWhirlpoolSqrtPrice);

        // Convert SqrtPrice Q64.64 to standard base-10 price scaled to 6 decimals
        let normalized_amm_price_u64 = calculate_whirlpool_price_scale6(
            sqrt_price_q64,
            expected_base_decimals,
            expected_quote_decimals,
        )?;

        // 3. Compute Divergence in Basis Points (bps)
        // Divergence BPS = (|Pyth - AMM| * 10_000) / Pyth
        let diff = if normalized_pyth_price_u64 >= normalized_amm_price_u64 {
            normalized_pyth_price_u64 - normalized_amm_price_u64
        } else {
            normalized_amm_price_u64 - normalized_pyth_price_u64
        };

        let calculated_divergence_bps = ((diff as u128)
            .checked_mul(BPS_DENOMINATOR as u128)
            .ok_or(PriceMatrixError::MathOverflow)?)
        .checked_div(normalized_pyth_price_u64 as u128)
        .ok_or(PriceMatrixError::MathDivisionByZero)? as u64;

        msg!(
            "Price Comparison Matrix: Pyth Price = {}, AMM Price = {}, Divergence = {} BPS",
            normalized_pyth_price_u64,
            normalized_amm_price_u64,
            calculated_divergence_bps
        );

        // Record verification state
        let log = &mut ctx.accounts.comparison_log;
        log.asset_feed = ctx.accounts.pyth_price_account.key();
        log.amm_pool = ctx.accounts.whirlpool_account.key();
        log.pyth_price_normalized = normalized_pyth_price_u64;
        log.amm_price_normalized = normalized_amm_price_u64;
        log.divergence_bps = calculated_divergence_bps as u16;
        log.evaluated_slot = current_slot;
        log.is_divergence_safe = calculated_divergence_bps <= ctx.accounts.matrix_config.max_allowed_divergence_bps as u64;

        require!(
            log.is_divergence_safe,
            PriceMatrixError::PriceDivergenceExceeded
        );

        Ok(())
    }
}

/// Helper function to convert raw oracle pricing with variable exponent to fixed scale (e.g. 10^6)
pub fn normalize_to_scale(raw_price: u64, expo: i32, target_scale: i32) -> Result<u64> {
    let diff = target_scale - expo;
    if diff == 0 {
        return Ok(raw_price);
    }

    if diff > 0 {
        let factor = 10u128.pow(diff as u32);
        let res = (raw_price as u128)
            .checked_mul(factor)
            .ok_or(PriceMatrixError::MathOverflow)?;
        Ok(res as u64)
    } else {
        let factor = 10u128.pow((-diff) as u32);
        let res = (raw_price as u128)
            .checked_div(factor)
            .ok_or(PriceMatrixError::MathDivisionByZero)?;
        Ok(res as u64)
    }
}

/// Convert Q64.64 sqrt_price to spot price scaled to 1e6
pub fn calculate_whirlpool_price_scale6(
    sqrt_price_q64: u128,
    base_decimals: u8,
    quote_decimals: u8,
) -> Result<u64> {
    // Sqrt Price is Q64.64.
    // Price = (sqrt_price / 2^64)^2 = (sqrt_price^2) / 2^128
    // To prevent intermediate u128 overflow:
    // Scale up by target precision 1e6
    let target_precision: u128 = 1_000_000;
    
    // sqrt_price_q64 / 2^32 gives 32.32
    // square that gives 64.64
    let q32 = sqrt_price_q64 >> 32;
    let squared = q32.checked_mul(q32).ok_or(PriceMatrixError::MathOverflow)?;
    
    // Divide out remaining 2^64
    let base_price = squared >> 64;

    // Apply decimal delta: quote_decimals - base_decimals
    let dec_delta = (quote_decimals as i32) - (base_decimals as i32);
    let final_price = if dec_delta >= 0 {
        let factor = 10u128.pow(dec_delta as u32);
        base_price
            .checked_mul(target_precision)
            .ok_or(PriceMatrixError::MathOverflow)?
            .checked_div(factor)
            .ok_or(PriceMatrixError::MathDivisionByZero)?
    } else {
        let factor = 10u128.pow((-dec_delta) as u32);
        base_price
            .checked_mul(target_precision)
            .ok_or(PriceMatrixError::MathOverflow)?
            .checked_mul(factor)
            .ok_or(PriceMatrixError::MathOverflow)?
    };

    Ok(final_price as u64)
}

#[derive(Accounts)]
pub struct InitializeMatrixConfig<'info> {
    #[account(
        init,
        payer = admin,
        space = 8 + MatrixConfig::INIT_SPACE,
        seeds = [b"matrix_config"],
        bump
    )]
    pub matrix_config: Account<'info, MatrixConfig>,
    #[account(mut)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CompareAndGuardPrice<'info> {
    #[account(
        seeds = [b"matrix_config"],
        bump = matrix_config.bump
    )]
    pub matrix_config: Account<'info, MatrixConfig>,
    /// CHECK: Validated via pyth_sdk_solana deserializer
    pub pyth_price_account: AccountInfo<'info>,
    /// CHECK: Validated byte offset checks for Orca Whirlpool
    pub whirlpool_account: AccountInfo<'info>,
    #[account(
        init_if_needed,
        payer = caller,
        space = 8 + PriceDiscrepancyLog::INIT_SPACE,
        seeds = [b"comparison_log", pyth_price_account.key().as_ref(), whirlpool_account.key().as_ref()],
        bump
    )]
    pub comparison_log: Account<'info, PriceDiscrepancyLog>,
    #[account(mut)]
    pub caller: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[account]
#[derive(InitSpace)]
pub struct MatrixConfig {
    pub admin: Pubkey,
    pub max_allowed_divergence_bps: u16,
    pub max_staleness_slots: u64,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct PriceDiscrepancyLog {
    pub asset_feed: Pubkey,
    pub amm_pool: Pubkey,
    pub pyth_price_normalized: u64,
    pub amm_price_normalized: u64,
    pub divergence_bps: u16,
    pub evaluated_slot: u64,
    pub is_divergence_safe: bool,
}

#[error_code]
pub enum PriceMatrixError {
    #[msg("Divergence threshold must be <= 1000 bps (10%)")]
    InvalidDivergenceBpsThreshold,
    #[msg("Staleness slots must be between 1 and 300 slots")]
    InvalidStalenessThreshold,
    #[msg("Failed to load Pyth price feed")]
    FailedToLoadPythFeed,
    #[msg("Pyth price feed is stale")]
    PythPriceStale,
    #[msg("Pyth returned an invalid or non-positive price")]
    InvalidOraclePrice,
    #[msg("Invalid Whirlpool account layout")]
    InvalidWhirlpoolAccountLayout,
    #[msg("Invalid Whirlpool sqrt price")]
    InvalidWhirlpoolSqrtPrice,
    #[msg("Math calculation resulted in overflow")]
    MathOverflow,
    #[msg("Math calculation resulted in zero division")]
    MathDivisionByZero,
    #[msg("Calculated price divergence between oracle and AMM exceeds safe bounds")]
    PriceDivergenceExceeded,
}