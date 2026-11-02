use anchor_lang::prelude::*;
use pyth_sdk_solana::state::SolanaPriceAccount;

declare_id!("Comp111111111111111111111111111111111111111");

pub const MAX_STALENESS_SECONDS: i64 = 60;
pub const BPS_DENOMINATOR: u64 = 10_000;

#[program]
pub mod solana_price_comparator {
    use super::*;

    pub fn initialize_registry(
        ctx: Context<InitializeRegistry>,
        max_allowed_bps_divergence: u64,
    ) -> Result<()> {
        require!(
            max_allowed_bps_divergence > 0 && max_allowed_bps_divergence <= 5_000,
            ComparatorError::InvalidParameter
        );

        let registry = &mut ctx.accounts.registry;
        registry.authority = ctx.accounts.authority.key();
        registry.max_allowed_bps_divergence = max_allowed_bps_divergence;
        registry.comparison_count = 0;
        registry.bump = ctx.bumps.registry;

        msg!(
            "Registry initialized. Max Bps Divergence: {}",
            max_allowed_bps_divergence
        );
        Ok(())
    }

    pub fn update_registry_parameters(
        ctx: Context<UpdateRegistryParameters>,
        new_max_bps_divergence: u64,
    ) -> Result<()> {
        require!(
            new_max_bps_divergence > 0 && new_max_bps_divergence <= 5_000,
            ComparatorError::InvalidParameter
        );

        let registry = &mut ctx.accounts.registry;
        registry.max_allowed_bps_divergence = new_max_bps_divergence;

        msg!(
            "Registry updated. New Max Bps Divergence: {}",
            new_max_bps_divergence
        );
        Ok(())
    }

    pub fn verify_and_record_comparison(
        ctx: Context<VerifyAndRecordComparison>,
        token_symbol: [u8; 8],
        candidate_prices: Vec<DexQuote>,
    ) -> Result<()> {
        require!(!candidate_prices.is_empty(), ComparatorError::EmptyQuotesArray);
        require!(candidate_prices.len() <= 10, ComparatorError::ExcessiveQuotesArray);

        let clock = Clock::get()?;
        let current_timestamp = clock.unix_timestamp;

        // Load and validate Pyth Oracle Benchmark Price
        let pyth_price_info = &ctx.accounts.pyth_oracle;
        let price_feed = SolanaPriceAccount::account_info_to_feed(pyth_price_info)
            .map_err(|_| ComparatorError::InvalidOracleAccount)?;

        let pyth_price_data = price_feed
            .get_price_no_older_than(current_timestamp, MAX_STALENESS_SECONDS)
            .ok_or(ComparatorError::StaleOraclePrice)?;

        require!(pyth_price_data.price > 0, ComparatorError::NonPositiveOraclePrice);
        let oracle_price_scaled = pyth_price_data.price as u64;
        let oracle_expo = pyth_price_data.expo;

        // Iterate candidate DEX prices and discover best (highest quote value per base unit)
        let mut best_dex = candidate_prices[0].dex_id;
        let mut best_price = candidate_prices[0].price_scaled;
        let mut best_expo = candidate_prices[0].expo;

        for quote in candidate_prices.iter() {
            require!(quote.price_scaled > 0, ComparatorError::InvalidQuotePrice);
            
            // Standardize compare price against best_price via exponent alignment
            let is_better = if quote.expo == best_expo {
                quote.price_scaled > best_price
            } else if quote.expo < best_expo {
                let factor = 10u64.pow((best_expo - quote.expo) as u32);
                quote.price_scaled > best_price.checked_mul(factor).unwrap_or(u64::MAX)
            } else {
                let factor = 10u64.pow((quote.expo - best_expo) as u32);
                quote.price_scaled.checked_mul(factor).unwrap_or(0) > best_price
            };

            if is_better {
                best_dex = quote.dex_id;
                best_price = quote.price_scaled;
                best_expo = quote.expo;
            }
        }

        // Align Best DEX Price and Oracle Benchmark to a standard 6-decimal scaling factor
        let normalized_oracle = normalize_to_scale_6(oracle_price_scaled, oracle_expo)?;
        let normalized_best = normalize_to_scale_6(best_price, best_expo)?;

        // Compute Absolute Basis Point Divergence: |best - oracle| / oracle * 10,000
        let diff = if normalized_best >= normalized_oracle {
            normalized_best.checked_sub(normalized_oracle).unwrap()
        } else {
            normalized_oracle.checked_sub(normalized_best).unwrap()
        };

        let divergence_bps = (diff as u128)
            .checked_mul(BPS_DENOMINATOR as u128)
            .ok_or(ComparatorError::MathOverflow)?
            .checked_div(normalized_oracle as u128)
            .ok_or(ComparatorError::MathOverflow)? as u64;

        let registry = &mut ctx.accounts.registry;
        require!(
            divergence_bps <= registry.max_allowed_bps_divergence,
            ComparatorError::PriceDivergenceExceeded
        );

        // Update state tracking
        let comparison_record = &mut ctx.accounts.comparison_record;
        comparison_record.registry = registry.key();
        comparison_record.token_symbol = token_symbol;
        comparison_record.benchmark_oracle_price = normalized_oracle;
        comparison_record.best_dex_id = best_dex;
        comparison_record.best_dex_price = normalized_best;
        comparison_record.divergence_bps = divergence_bps;
        comparison_record.timestamp = current_timestamp;
        comparison_record.bump = ctx.bumps.comparison_record;

        registry.comparison_count = registry
            .comparison_count
            .checked_add(1)
            .ok_or(ComparatorError::MathOverflow)?;

        emit!(PriceComparisonVerifiedEvent {
            symbol: token_symbol,
            best_dex,
            best_price: normalized_best,
            oracle_price: normalized_oracle,
            divergence_bps,
            timestamp: current_timestamp,
        });

        msg!(
            "Price verified! Best DEX: {}, Price: {}, Oracle: {}, Div BPS: {}",
            best_dex,
            normalized_best,
            normalized_oracle,
            divergence_bps
        );

        Ok(())
    }
}

// Helper calculation to normalize arbitrary exponent prices to 10^6
fn normalize_to_scale_6(price: u64, expo: i32) -> Result<u64> {
    const TARGET_EXPO: i32 = -6;
    let delta = TARGET_EXPO - expo;
    if delta == 0 {
        Ok(price)
    } else if delta < 0 {
        // Current expo is closer to zero or positive (e.g. -4 vs -6 -> delta = -2)
        let factor = 10u64.pow((-delta) as u32);
        price.checked_mul(factor).ok_or(ComparatorError::MathOverflow.into())
    } else {
        // Current expo has more decimals (e.g. -8 vs -6 -> delta = 2)
        let factor = 10u64.pow(delta as u32);
        price.checked_div(factor).ok_or(ComparatorError::MathOverflow.into())
    }
}

// ---------------------- DATA STRUCTURES ----------------------

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub struct DexQuote {
    pub dex_id: u8, // 1 = Raydium, 2 = Orca, 3 = Phoenix, 4 = Meteora
    pub price_scaled: u64,
    pub expo: i32,
    pub volume_depth_usd: u64,
}

#[account]
pub struct ComparisonRegistry {
    pub authority: Pubkey,
    pub max_allowed_bps_divergence: u64,
    pub comparison_count: u64,
    pub bump: u8,
}

impl ComparisonRegistry {
    pub const LEN: usize = 8 + 32 + 8 + 8 + 1;
}

#[account]
pub struct ComparisonRecord {
    pub registry: Pubkey,
    pub token_symbol: [u8; 8],
    pub benchmark_oracle_price: u64,
    pub best_dex_id: u8,
    pub best_dex_price: u64,
    pub divergence_bps: u64,
    pub timestamp: i64,
    pub bump: u8,
}

impl ComparisonRecord {
    pub const LEN: usize = 8 + 32 + 8 + 8 + 1 + 8 + 8 + 8 + 1;
}

// ---------------------- ACCOUNTS ----------------------

#[derive(Accounts)]
pub fn initialize_registry_accounts<'info>(
    ctx: &Context<InitializeRegistry>,
) -> &'info mut Account<'info, ComparisonRegistry> {
    &mut ctx.accounts.registry
}

#[derive(Accounts)]
pub struct InitializeRegistry<'info> {
    #[account(
        init,
        payer = authority,
        space = ComparisonRegistry::LEN,
        seeds = [b"price_registry", authority.key().as_ref()],
        bump
    )]
    pub registry: Account<'info, ComparisonRegistry>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateRegistryParameters<'info> {
    #[account(
        mut,
        has_one = authority @ ComparatorError::Unauthorized
    )]
    pub registry: Account<'info, ComparisonRegistry>,
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
#[instruction(token_symbol: [u8; 8])]
pub struct VerifyAndRecordComparison<'info> {
    #[account(
        mut,
        seeds = [b"price_registry", registry.authority.as_ref()],
        bump = registry.bump
    )]
    pub registry: Account<'info, ComparisonRegistry>,

    #[account(
        init_if_needed,
        payer = payer,
        space = ComparisonRecord::LEN,
        seeds = [b"comparison_record", registry.key().as_ref(), token_symbol.as_ref()],
        bump
    )]
    pub comparison_record: Account<'info, ComparisonRecord>,

    /// CHECK: Validated via pyth_sdk_solana inside instruction logic
    pub pyth_oracle: AccountInfo<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

// ---------------------- EVENTS & ERRORS ----------------------

#[event]
pub struct PriceComparisonVerifiedEvent {
    pub symbol: [u8; 8],
    pub best_dex: u8,
    pub best_price: u64,
    pub oracle_price: u64,
    pub divergence_bps: u64,
    pub timestamp: i64,
}

#[error_code]
pub enum ComparatorError {
    #[msg("Provided parameter value is invalid.")]
    InvalidParameter,
    #[msg("Caller is unauthorized to mutate registry.")]
    Unauthorized,
    #[msg("Candidate quotes array cannot be empty.")]
    EmptyQuotesArray,
    #[msg("Exceeded maximum quotes limit per batch.")]
    ExcessiveQuotesArray,
    #[msg("Invalid Pyth oracle account provided.")]
    InvalidOracleAccount,
    #[msg("Pyth oracle price is stale or not published.")]
    StaleOraclePrice,
    #[msg("Oracle reported non-positive price.")]
    NonPositiveOraclePrice,
    #[msg("Candidate DEX quote has non-positive price.")]
    InvalidQuotePrice,
    #[msg("Calculated price divergence exceeds protocol maximum threshold.")]
    PriceDivergenceExceeded,
    #[msg("Math arithmetic overflow occurred.")]
    MathOverflow,
}