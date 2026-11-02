// On-chain deterministic spread assertion
let divergence_bps = (diff * 10_000) / oracle_price;
require!(divergence_bps <= max_allowed_divergence_bps, GuardError::DivergenceToleranceExceeded);