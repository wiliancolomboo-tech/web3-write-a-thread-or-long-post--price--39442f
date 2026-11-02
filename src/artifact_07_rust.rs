use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};
use pyth_solana_receiver_sdk::price_update::{get_feed_id_from_hex, PriceUpdateV2};

declare_id!("GuARd11111111111111111111111111111111111111");

pub const MAXIMUM_AGE_SECONDS: u64 = 60; // 60s max price age
pub const BPS_DENOMINATOR: u64 = 10_000;

#[program]
pub mod price_comparator_guard {
    use super::*;

    /// Initializes a guard profile defining security thresholds for institutional pairs.
    pub fn initialize_guard(
        ctx: Context<InitializeGuard>,
        feed_id_hex: String,
        max_deviation_bps: u64,
        max_confidence_pct_bps: u64,
    ) -> Result<()> {
        require!(max_deviation_bps <= 1_000, GuardError::DeviationThresholdTooHigh); // Max 10%
        require!(max_confidence_pct_bps <= 2_000, GuardError::InvalidConfidenceThreshold);

        let guard = &mut ctx.accounts.guard_config;
        guard.authority = ctx.accounts.authority.key();
        guard.feed_id = get_feed_id_from_hex(&feed_id_hex)
            .map_err(|_| GuardError::InvalidFeedId)?;
        guard.max_deviation_bps = max_deviation_bps;
        guard.max_confidence_pct_bps = max_confidence_pct_bps;
        guard.total_inspections = 0;
        guard.total_rejections = 0;
        guard.bump = ctx.bumps.guard_config;

        msg!(
            "Guard initialized for feed {:?}, max deviation: {} bps",
            feed_id_hex,
            max_deviation_bps
        );
        Ok(())
    }

    /// Verifies that an incoming DEX price quotation is consistent with Pyth pull feed price
    /// within tolerable bounds before allowing downstream settlement.
    pub fn verify_price_execution(
        ctx: Context<VerifyPriceExecution>,
        quoted_price_scaled: u64, // Scaled with Pyth exponent, e.g., $150.25 -> 15025000000 with exp -8
        min_output_tokens: u64,
        actual_output_tokens: u64,
    ) -> Result<()> {
        let guard = &mut ctx.accounts.guard_config;
        let clock = Clock::get()?;

        // 1. Fetch deterministic Pyth Price Update
        let price_update = &ctx.accounts.price_update;
        let current_price = price_update.get_price_no_older_than(
            &clock,
            MAXIMUM_AGE_SECONDS,
            &guard.feed_id,
        ).map_err(|_| GuardError::StaleOrInvalidPrice)?;

        require!(current_price.price > 0, GuardError::NegativePriceDetected);
        let pyth_price = current_price.price as u64;
        let pyth_conf = current_price.conf as u64;

        // 2. Enforce confidence interval width: conf / price <= max_confidence_pct_bps
        let conf_pct_bps = (pyth_conf as u128 * BPS_DENOMINATOR as u128)
            .checked_div(pyth_price as u128)
            .ok_or(GuardError::MathOverflow)?;

        if conf_pct_bps > guard.max_confidence_pct_bps as u128 {
            guard.total_rejections = guard.total_rejections.saturating_add(1);
            return Err(GuardError::ConfidenceIntervalTooWide.into());
        }

        // 3. Compute Price Deviation between Quoted DEX Price and Oracle Baseline
        let diff = if quoted_price_scaled >= pyth_price {
            quoted_price_scaled - pyth_price
        } else {
            pyth_price - quoted_price_scaled
        };

        let deviation_bps = (diff as u128 * BPS_DENOMINATOR as u128)
            .checked_div(pyth_price as u128)
            .ok_or(GuardError::MathOverflow)?;

        msg!(
            "Price Validation: DEX Quoted={}, Pyth={}, Diff={}, DevBps={}",
            quoted_price_scaled,
            pyth_price,
            diff,
            deviation_bps
        );

        if deviation_bps > guard.max_deviation_bps as u128 {
            guard.total_rejections = guard.total_rejections.saturating_add(1);
            msg!(
                "Price discrepancy breach: {} bps > limit of {} bps",
                deviation_bps,
                guard.max_deviation_bps
            );
            return Err(GuardError::PriceDiscrepancyExceeded.into());
        }

        // 4. Assert minimum output slippage satisfaction
        require!(
            actual_output_tokens >= min_output_tokens,
            GuardError::SlippageExceeded
        );

        guard.total_inspections = guard.total_inspections.saturating_add(1);

        emit!(PriceComparisonVerifiedEvent {
            feed_id: guard.feed_id,
            dex_price: quoted_price_scaled,
            pyth_price,
            deviation_bps: deviation_bps as u64,
            executed_tokens: actual_output_tokens,
            timestamp: clock.unix_timestamp,
        });

        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(feed_id_hex: String)]
pub懸
pub struct InitializeGuard<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + GuardConfig::LEN,
        seeds = [b"guard_config", authority.key().as_ref(), feed_id_hex.as_bytes()],
        bump
    )]
    pub guard_config: Account<'info, GuardConfig>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct VerifyPriceExecution<'info> {
    #[account(
        mut,
        has_one = authority,
        seeds = [b"guard_config", authority.key().as_ref(), &guard_config.feed_id],
        bump = guard_config.bump
    )]
    pub guard_config: Account<'info, GuardConfig>,
    pub authority: Signer<'info>,
    pub price_update: Account<'info, PriceUpdateV2>,
}

#[account]
pub struct GuardConfig {
    pub authority: Pubkey,            // 32
    pub feed_id: [u8; 32],            // 32
    pub max_deviation_bps: u64,       // 8
    pub max_confidence_pct_bps: u64,  // 8
    pub total_inspections: u64,       // 8
    pub total_rejections: u64,        // 8
    pub bump: u8,                     // 1
}

impl GuardConfig {
    pub const LEN: usize = 32 + 32 + 8 + 8 + 8 + 8 + 1;
}

#[event]
pub struct PriceComparisonVerifiedEvent {
    pub feed_id: [u8; 32],
    pub dex_price: u64,
    pub pyth_price: u64,
    pub deviation_bps: u64,
    pub executed_tokens: u64,
    pub timestamp: i64,
}

#[error_code]
pub enum GuardError {
    #[msg("Deviation threshold is higher than permitted safety limits (1000 bps).")]
    DeviationThresholdTooHigh,
    #[msg("Invalid confidence percentage threshold specified.")]
    InvalidConfidenceThreshold,
    #[msg("Invalid 32-byte hex feed ID supplied.")]
    InvalidFeedId,
    #[msg("Oracle price feed is older than acceptable tolerance.")]
    StaleOrInvalidPrice,
    #[msg("Reported oracle price is negative or zero.")]
    NegativePriceDetected,
    #[msg("Pyth confidence interval is too wide to permit safe execution.")]
    ConfidenceIntervalTooWide,
    #[msg("Price discrepancy between DEX route and oracle baseline exceeds threshold.")]
    PriceDiscrepancyExceeded,
    #[msg("Output token amount is less than caller minimum expected amount.")]
    SlippageExceeded,
    #[msg("Math arithmetic overflow during scaling operations.")]
    MathOverflow,
}