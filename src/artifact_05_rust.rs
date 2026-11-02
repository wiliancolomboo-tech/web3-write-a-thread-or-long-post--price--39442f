use anchor_lang::prelude::*;
use pyth_sdk_solana::load_price_feed_from_account_info;

declare_id!("PriceComp111111111111111111111111111111111");

pub const MAX_ALLOWED_STALENESS_SECONDS: u64 = 60;
pub const BPS_DENOMINATOR: u64 = 10_000;

#[program]
pub mod sol_price_compare {
    use super::*;

    /// Initialize the Global Comparison Registry configuration
    pub fn initialize_registry(
        ctx: Context<InitializeRegistry>,
        divergence_alert_threshold_bps: u16,
    ) -> Result<()> {
        let registry = &mut ctx.accounts.registry;
        registry.authority = ctx.accounts.authority.key();
        registry.divergence_alert_threshold_bps = divergence_alert_threshold_bps;
        registry.total_attestations = 0;
        registry.bump = ctx.bumps.registry;
        msg!(
            "Registry initialized by {}. Divergence threshold: {} bps",
            registry.authority,
            divergence_alert_threshold_bps
        );
        Ok(())
    }

    /// Record a verified price comparison between two execution venues and benchmark with Pyth
    pub fn record_price_comparison(
        ctx: Context<RecordPriceComparison>,
        token_pair: [u8; 16],
        venue_a_price: u64, // Scaled by 1e6
        venue_b_price: u64, // Scaled by 1e6
        client_timestamp: i64,
    ) -> Result<()> {
        require!(venue_a_price > 0, PriceCompareError::InvalidPrice);
        require!(venue_b_price > 0, PriceCompareError::InvalidPrice);

        // 1. Verify Pyth Oracle Price and Staleness
        let pyth_account_info = &ctx.accounts.pyth_price_feed;
        let price_feed = load_price_feed_from_account_info(pyth_account_info)
            .map_err(|_| PriceCompareError::InvalidPythFeed)?;
        
        let clock = Clock::get()?;
        let current_timestamp = clock.unix_timestamp;

        let pyth_price_data = price_feed
            .get_price_no_older_than(current_timestamp, MAX_ALLOWED_STALENESS_SECONDS)
            .ok_or(PriceCompareError::PythFeedStale)?;

        require!(pyth_price_data.price > 0, PriceCompareError::InvalidPythPrice);
        let pyth_price_normalized = pyth_price_data.price as u64;

        // 2. Calculate Basis Point Divergence between Venue A and Venue B
        let abs_diff = if venue_a_price >= venue_b_price {
            venue_a_price - venue_b_price
        } else {
            venue_b_price - venue_a_price
        };

        let min_price = venue_a_price.min(venue_b_price);
        let spread_bps = (abs_diff as u128)
            .checked_mul(BPS_DENOMINATOR as u128)
            .and_then(|val| val.checked_div(min_price as u128))
            .ok_or(PriceCompareError::MathOverflow)? as u64;

        // 3. Store Attestation in PDA
        let attestation = &mut ctx.accounts.attestation;
        attestation.registry = ctx.accounts.registry.key();
        attestation.token_pair = token_pair;
        attestation.venue_a_price = venue_a_price;
        attestation.venue_b_price = venue_b_price;
        attestation.pyth_benchmark_price = pyth_price_normalized;
        attestation.pyth_confidence = pyth_price_data.conf;
        attestation.spread_bps = spread_bps;
        attestation.timestamp = current_timestamp;
        attestation.attester = ctx.accounts.attester.key();
        attestation.bump = ctx.bumps.attestation;

        let registry = &mut ctx.accounts.registry;
        registry.total_attestations = registry
            .total_attestations
            .checked_add(1)
            .ok_or(PriceCompareError::MathOverflow)?;

        emit!(PriceComparisonLogged {
            token_pair,
            venue_a_price,
            venue_b_price,
            pyth_benchmark_price: pyth_price_normalized,
            spread_bps,
            timestamp: current_timestamp,
            attester: ctx.accounts.attester.key(),
        });

        if spread_bps >= (registry.divergence_alert_threshold_bps as u64) {
            emit!(DivergenceThresholdExceeded {
                token_pair,
                spread_bps,
                threshold_bps: registry.divergence_alert_threshold_bps,
                timestamp: current_timestamp,
            });
        }

        Ok(())
    }
}

// -----------------------------------------------------------------------------
// Account Contexts
// -----------------------------------------------------------------------------

#[derive(Accounts)]
pub struct InitializeRegistry<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + ComparisonRegistry::LEN,
        seeds = [b"price_registry"],
        bump
    )]
    pub registry: Account<'info, ComparisonRegistry>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(token_pair: [u8; 16], venue_a_price: u64, venue_b_price: u64, client_timestamp: i64)]
pub struct RecordPriceComparison<'info> {
    #[account(
        mut,
        seeds = [b"price_registry"],
        bump = registry.bump
    )]
    pub registry: Account<'info, ComparisonRegistry>,

    #[account(
        init,
        payer = attester,
        space = 8 + PriceAttestation::LEN,
        seeds = [
            b"attestation",
            registry.key().as_ref(),
            token_pair.as_ref(),
            &client_timestamp.to_le_bytes()
        ],
        bump
    )]
    pub attestation: Account<'info, PriceAttestation>,

    /// CHECK: Validated manually inside handler via pyth_sdk_solana deserialization
    pub pyth_price_feed: AccountInfo<'info>,

    #[account(mut)]
    pub attester: Signer<'info>,
    pub system_program: Program<'info, System>,
}

// -----------------------------------------------------------------------------
// State Accounts
// -----------------------------------------------------------------------------

#[account]
pub struct ComparisonRegistry {
    pub authority: Pubkey,                     // 32
    pub divergence_alert_threshold_bps: u16,   // 2
    pub total_attestations: u64,               // 8
    pub bump: u8,                              // 1
}

impl ComparisonRegistry {
    pub const LEN: usize = 32 + 2 + 8 + 1;
}

#[account]
pub struct PriceAttestation {
    pub registry: Pubkey,                      // 32
    pub token_pair: [u8; 16],                  // 16
    pub venue_a_price: u64,                    // 8
    pub venue_b_price: u64,                    // 8
    pub pyth_benchmark_price: u64,             // 8
    pub pyth_confidence: u64,                  // 8
    pub spread_bps: u64,                       // 8
    pub timestamp: i64,                        // 8
    pub attester: Pubkey,                      // 32
    pub bump: u8,                              // 1
}

impl PriceAttestation {
    pub const LEN: usize = 32 + 16 + 8 + 8 + 8 + 8 + 8 + 8 + 32 + 1;
}

// -----------------------------------------------------------------------------
// Events & Errors
// -----------------------------------------------------------------------------

#[event]
pub struct PriceComparisonLogged {
    pub token_pair: [u8; 16],
    pub venue_a_price: u64,
    pub venue_b_price: u64,
    pub pyth_benchmark_price: u64,
    pub spread_bps: u64,
    pub timestamp: i64,
    pub attester: Pubkey,
}

#[event]
pub struct DivergenceThresholdExceeded {
    pub token_pair: [u8; 16],
    pub spread_bps: u64,
    pub threshold_bps: u16,
    pub timestamp: i64,
}

#[error_code]
pub enum PriceCompareError {
    #[msg("Input venue price cannot be zero.")]
    InvalidPrice,
    #[msg("Provided account failed Pyth SDK deserialization.")]
    InvalidPythFeed,
    #[msg("Pyth price feed exceeds allowable staleness window.")]
    PythFeedStale,
    #[msg("Pyth price must be positive.")]
    InvalidPythPrice,
    #[msg("Arithmetic overflow in divergence calculation.")]
    MathOverflow,
}