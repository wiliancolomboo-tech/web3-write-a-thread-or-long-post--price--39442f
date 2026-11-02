use solcompare_core::comparator::PriceComparator;
use solcompare_core::math::cpmm::compute_cpmm_quote;
use solcompare_core::math::tick_math::{compute_clmm_swap_step, tick_to_sqrt_price_x64, MIN_TICK, MAX_TICK};
use solcompare_core::models::{DexType, PoolState};

#[test]
fn test_tick_to_sqrt_price_boundaries() {
    // Exact midpoint tick (0) must equal 2^64 (fixed point 1.0)
    let price_zero = tick_to_sqrt_price_x64(0).expect("Tick 0 must be valid");
    assert_eq!(price_zero, 1u128 << 64);

    // Ensure bound validation triggers on extreme values
    assert!(tick_to_sqrt_price_x64(MIN_TICK - 1).is_err());
    assert!(tick_to_sqrt_price_x64(MAX_TICK + 1).is_err());
}

#[test]
fn test_cpmm_deterministic_pricing_and_fee() {
    let reserve_in = 1_000_000_000u64; // 1,000 units
    let reserve_out = 50_000_000_000u64; // 50,000 units
    let amount_in = 10_000_000u64; // 10 units
    let fee_rate_bps = 30; // 0.3%

    let (amount_out, fee_amount, price_impact_bps) =
        compute_cpmm_quote(reserve_in, reserve_out, amount_in, fee_rate_bps)
            .expect("Quote calculation must succeed");

    assert!(amount_out > 0);
    assert_eq!(fee_amount, 30_000); // 10,000,000 * 30 / 10,000
    assert!(price_impact_bps > 0.0);
    assert!(price_impact_bps < 100.0); // Less than 1% impact for this proportion
}

#[test]
fn test_cross_dex_price_comparison_ranking() {
    let pools = vec![
        PoolState {
            dex_type: DexType::OrcaWhirlpool,
            pool_address: "OrcaPool11111111111111111111111111111111111".to_string(),
            token_mint_a: "So11111111111111111111111111111111111111112".to_string(),
            token_mint_b: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
            fee_rate_bps: 4,
            reserve_a: 500_000_000_000,
            reserve_b: 75_000_000_000_000,
            sqrt_price_x64: 1u128 << 64,
            current_tick: 0,
            liquidity: 10_000_000_000_000,
        },
        PoolState {
            dex_type: DexType::RaydiumCpmm,
            pool_address: "RaydiumPool111111111111111111111111111111111".to_string(),
            token_mint_a: "So11111111111111111111111111111111111111112".to_string(),
            token_mint_b: "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".to_string(),
            fee_rate_bps: 25,
            reserve_a: 100_000_000_000,
            reserve_b: 14_000_000_000_000,
            sqrt_price_x64: 1u128 << 64,
            current_tick: 0,
            liquidity: 5_000_000_000_000,
        },
    ];

    let report = PriceComparator::evaluate_pools(
        "SOL",
        "USDC",
        1_000_000_000,
        &pools,
        5000, // 5000 micro-lamports per CU
    )
    .expect("Comparison must succeed");

    assert_eq!(report.quotes.len(), 2);
    assert_eq!(report.best_dex, DexType::OrcaWhirlpool);
    assert!(report.bps_improvement_vs_worst > 0.0);
    assert!(report.absolute_savings_vs_worst > 0);
}