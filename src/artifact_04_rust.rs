use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};

declare_id!("Pr1smGuard1111111111111111111111111111111111");

#[program]
pub mod solprism_guard {
    use super::*;

    /// Initializes a price guard instance for an institutional trading desk
    pub fn initialize_guard(
        ctx: Context<InitializeGuard>,
        max_divergence_bps: u16,
    ) -> Result<()> {
        require!(max_divergence_bps <= 1000, SolPrismError::InvalidDivergenceBound); // Max 10%

        let guard_config = &mut ctx.accounts.guard_config;
        guard_config.authority = ctx.accounts.authority.key();
        guard_config.max_divergence_bps = max_divergence_bps;
        guard_config.total_verifications = 0;
        guard_config.is_paused = false;

        emit!(GuardInitializedEvent {
            authority: ctx.accounts.authority.key(),
            max_divergence_bps,
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }

    /// Compares two independent price feeds (e.g. Venue A vs Venue B) and verifies that
    /// the route satisfies the maximum allowable price divergence.
    pub fn verify_and_route_quote(
        ctx: Context<VerifyAndRoute>,
        venue_a_out_amount: u64,
        venue_b_out_amount: u64,
        min_expected_out: u64,
    ) -> Result<()> {
        let guard_config = &mut ctx.accounts.guard_config;
        require!(!guard_config.is_paused, SolPrismError::GuardCircuitBreakerActive);

        // Best execution selection
        let best_out_amount = std::cmp::max(venue_a_out_amount, venue_b_out_amount);
        let worst_out_amount = std::cmp::min(venue_a_out_amount, venue_b_out_amount);

        require!(best_out_amount >= min_expected_out, SolPrismError::SlippageExceeded);

        // Mathematical Divergence Calculation in Basis Points (bps)
        // delta_bps = ((Best - Worst) * 10,000) / Best
        let divergence_bps: u64 = best_out_amount
            .checked_sub(worst_out_amount)
            .ok_or(SolPrismError::MathOverflow)?
            .checked_mul(10_000)
            .ok_or(SolPrismError::MathOverflow)?
            .checked_div(best_out_amount)
            .ok_or(SolPrismError::MathOverflow)?;

        // Ensure market discrepancy is within allowable bounds
        require!(
            divergence_bps <= guard_config.max_divergence_bps as u64,
            SolPrismError::ExcessivePriceDivergence
        );

        guard_config.total_verifications = guard_config
            .total_verifications
            .checked_add(1)
            .ok_or(SolPrismError::MathOverflow)?;

        emit!(PriceComparisonVerifiedEvent {
            venue_a_out: venue_a_out_amount,
            venue_b_out: venue_b_out_amount,
            selected_best_out: best_out_amount,
            divergence_bps: divergence_bps as u16,
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }

    /// Admin emergency circuit breaker
    pub fn set_circuit_breaker(ctx: Context<ManageGuard>, pause: bool) -> Result<()> {
        let guard_config = &mut ctx.accounts.guard_config;
        guard_config.is_paused = pause;

        emit!(CircuitBreakerToggledEvent {
            is_paused: pause,
            authority: ctx.accounts.authority.key(),
            timestamp: Clock::get()?.unix_timestamp,
        });

        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitializeGuard<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + GuardConfig::INIT_SPACE,
        seeds = [b"solprism_guard", authority.key().as_ref()],
        bump
    )]
    pub guard_config: Account<'info, GuardConfig>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct VerifyAndRoute<'info> {
    #[account(
        mut,
        seeds = [b"solprism_guard", guard_config.authority.as_ref()],
        bump
    )]
    pub guard_config: Account<'info, GuardConfig>,
    pub caller: Signer<'info>,
}

#[derive(Accounts)]
pub struct ManageGuard<'info> {
    #[account(
        mut,
        has_one = authority @ SolPrismError::UnauthorizedCaller
    )]
    pub guard_config: Account<'info, GuardConfig>,
    pub authority: Signer<'info>,
}

#[account]
#[derive(InitSpace)]
pub struct GuardConfig {
    pub authority: Pubkey,
    pub max_divergence_bps: u16,
    pub total_verifications: u64,
    pub is_paused: bool,
}

#[event]
pub struct GuardInitializedEvent {
    pub authority: Pubkey,
    pub max_divergence_bps: u16,
    pub timestamp: i64,
}

#[event]
pub struct PriceComparisonVerifiedEvent {
    pub venue_a_out: u64,
    pub venue_b_out: u64,
    pub selected_best_out: u64,
    pub divergence_bps: u16,
    pub timestamp: i64,
}

#[event]
pub struct CircuitBreakerToggledEvent {
    pub is_paused: bool,
    pub authority: Pubkey,
    pub timestamp: i64,
}

#[error_code]
pub enum SolPrismError {
    #[msg("Maximum divergence bps exceeds safety cap of 1000 bps (10%)")]
    InvalidDivergenceBound,
    #[msg("Execution halted: SolPrism circuit breaker is active")]
    GuardCircuitBreakerActive,
    #[msg("Calculated output is strictly less than minimum allowable output")]
    SlippageExceeded,
    #[msg("Mathematical overflow occurred during quotation analysis")]
    MathOverflow,
    #[msg("Price divergence between venues exceeds the safety threshold")]
    ExcessivePriceDivergence,
    #[msg("Caller is unauthorized to mutate guard configuration")]
    UnauthorizedCaller,
}