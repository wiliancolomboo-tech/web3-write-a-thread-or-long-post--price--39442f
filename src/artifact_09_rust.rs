#[cfg(test)]
mod tests {
    use sol_price_guard_core::comparator::PriceComparator;
    use sol_price_guard_core::math::PriceMath;
    use sol_price_guard_core::types::{PriceQuote, VenueType};

    #[test]
    fn test_spread_bps_calculation() {
        let price_a = 100.0;
        let price_b = 101.0;
        let spread = PriceMath::calculate_spread_bps(price_a, price_b);
        // (101 - 100) / 100 * 10000 = 100 bps
        assert!((spread - 100.0).abs() < 1e-6);
    }

    #[test]
    fn test_zero_or_negative_price_handling() {
        let spread = PriceMath::calculate_spread_bps(0.0, 100.0);
        assert_eq!(spread, 0.0);

        let spread_neg = PriceMath::calculate_spread_bps(-10.0, 100.0);
        assert_eq!(spread_neg, 0.0);
    }

    #[test]
    fn test_sqrt_price_x64_conversion() {
        // Test 1:1 ratio for identical decimals (sqrt(1) * 2^64)
        let one_x64 = 1u128 << 64;
        let price = PriceMath::sqrt_price_x64_to_price(one_x64, 6, 6);
        assert!((price - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_arbitrage_viability_check() {
        let spread = 45.0; // 45 bps
        let fees = 25.0;   // 25 bps
        assert!(PriceMath::is_arbitrage_profitable(spread, fees));

        let small_spread = 15.0;
        assert!(!PriceMath::is_arbitrage_profitable(small_spread, fees));
    }

    #[test]
    fn test_price_matrix_evaluation() {
        let quotes = vec![
            PriceQuote {
                venue: VenueType::PhoenixCLOB,
                token_pair: "SOL/USDC".into(),
                price: 150.00,
                confidence_or_spread: 0.01,
                liquidity_depth_usd: 500_000.0,
                slot: 1000,
                timestamp_ms: 1000,
            },
            PriceQuote {
                venue: VenueType::OrcaWhirlpool,
                token_pair: "SOL/USDC".into(),
                price: 151.50,
                confidence_or_spread: 0.01,
                liquidity_depth_usd: 800_000.0,
                slot: 1000,
                timestamp_ms: 1000,
            },
            PriceQuote {
                venue: VenueType::PythOracle,
                token_pair: "SOL/USDC".into(),
                price: 150.50,
                confidence_or_spread: 0.02,
                liquidity_depth_usd: 2_000_000.0,
                slot: 1000,
                timestamp_ms: 1000,
            },
        ];

        let result = PriceComparator::evaluate(quotes).expect("Evaluation should succeed");

        assert_eq!(result.min_price_venue, VenueType::PhoenixCLOB);
        assert_eq!(result.max_price_venue, VenueType::OrcaWhirlpool);
        assert_eq!(result.best_execution_venue, VenueType::PhoenixCLOB);
        // (151.50 - 150.00) / 150.00 * 10,000 = 100 bps
        assert!((result.spread_bps - 100.0).abs() < 1e-4);
        assert!(result.is_arbitrage_viable);
    }
}