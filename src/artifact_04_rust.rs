//! Path: /tests/math_tests.rs

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::price_comparator_engine::*;

    #[test]
    fn test_constant_product_exact_price() {
        // Reserve A: 1,000 SOL (9 decimals) -> 1,000 * 10^9 = 1,000,000,000,000
        // Reserve B: 150,000 USDC (6 decimals) -> 150,000 * 10^6 = 150,000,000,000
        // Expected price = 150 USDC per SOL -> 150 * 1e12 = 150_000_000_000_000
        let pool = PoolStateSnapshot {
            pool_address: [1u8; 32],
            pool_type: PoolType::ConstantProduct,
            token_a_decimals: 9,
            token_b_decimals: 6,
            sqrt_price_x64: 0,
            tick_current: 0,
            reserve_a: 1_000_000_000_000,
            reserve_b: 150_000_000_000,
            fee_bps: 25,
        };

        let price = calculate_effective_price(&pool).expect("Price calculation failed");
        assert_eq!(price, 150_000_000_000_000);
    }

    #[test]
    fn test_zero_reserves_failure() {
        let pool = PoolStateSnapshot {
            pool_address: [0u8; 32],
            pool_type: PoolType::ConstantProduct,
            token_a_decimals: 9,
            token_b_decimals: 6,
            sqrt_price_x64: 0,
            tick_current: 0,
            reserve_a: 0,
            reserve_b: 150_000_000,
            fee_bps: 25,
        };

        assert!(calculate_effective_price(&pool).is_err());
    }

    #[test]
    fn test_pyth_normalization_and_divergence() {
        // Pyth price: 150.25 USD, expo: -8
        // raw price = 15025000000
        let oracle = PythOracleSnapshot {
            price: 15025000000,
            conf: 50000000, // 0.50 USD confidence
            expo: -8,
            publish_time: 1710000000,
        };

        let (scaled_p, scaled_c) = normalize_pyth_price(&oracle).unwrap();
        // Target is 1e12 scale. 150.25 * 10^12 = 150_250_000_000_000
        assert_eq!(scaled_p, 150_250_000_000_000);
        assert_eq!(scaled_c, 500_000_000_000);

        // Test with pool at 150.00
        let pool = PoolStateSnapshot {
            pool_address: [2u8; 32],
            pool_type: PoolType::ConstantProduct,
            token_a_decimals: 9,
            token_b_decimals: 6,
            sqrt_price_x64: 0,
            tick_current: 0,
            reserve_a: 1_000_000_000_000,
            reserve_b: 150_000_000_000,
            fee_bps: 25,
        };

        let result = compare_pool_vs_oracle(&pool, &oracle).unwrap();
        // Diff = 0.25 USD on 150.25 = ~16.6 bps
        assert_eq!(result.absolute_divergence_bps, 16);
        assert!(result.is_within_oracle_confidence);
    }
}