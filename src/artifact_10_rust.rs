use anchor_lang::prelude::*;

pub mod errors;
pub mod instructions;
pub mod state;

use instructions::*;

declare_id!("Pr1ceVer1f1er11111111111111111111111111111");

#[program]
pub mod sol_price_verifier {
    use super::*;

    pub fn compare_and_record(
        ctx: Context<CompareAndRecord>,
        token_pair_hash: [u8; 32],
        venue_a_id: u8,
        price_venue_a: u64,
        venue_b_id: u8,
        price_venue_b: u64,
        max_tolerated_spread_bps: u32,
    ) -> Result<()> {
        instructions::compare_and_record::handler(
            ctx,
            token_pair_hash,
            venue_a_id,
            price_venue_a,
            venue_b_id,
            price_venue_b,
            max_tolerated_spread_bps,
        )
    }
}