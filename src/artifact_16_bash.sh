$ npm test
  SolPriceCompare Mathematical & Guard Engine Test Suite
    ✔ should accurately convert Orca SqrtPriceX64 to Spot Price (1ms)
    ✔ should deterministically calculate divergence basis points (0ms)
    ✔ should reject feeds when oracle confidence exceeds threshold (0ms)
    ✔ should normalize Pyth raw integer and exponent to 1e6 decimal standard (0ms)
    ✔ should flag stale prices older than max_staleness_seconds (0ms)

  5 passing (12ms)