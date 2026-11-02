let cur_sqrt_p = (pool.sqrt_price_x64 as f64) / (Q64 as f64);
let delta_sqrt_p = (net_quote_in * 1e6) / (pool.liquidity as f64);
let next_sqrt_p = cur_sqrt_p + delta_sqrt_p;
let effective_price = quote_in_usd / net_base_received;