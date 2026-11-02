require!(
    min_expected_out_amount >= lower_bound,
    PriceGuardError::PriceDivergenceExceeded
);