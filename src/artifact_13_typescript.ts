import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { PriceComparisonGuard } from "../target/types/price_comparison_guard";
import { PublicKey, Keypair, SystemProgram } from "@solana/web3.js";
import { expect } from "chai";

describe("Price Comparison Guard Smart Contract Suite", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.PriceComparisonGuard as Program<PriceComparisonGuard>;
  const admin = (provider.wallet as any).payer as Keypair;

  const baseMint = Keypair.generate().publicKey;
  const quoteMint = Keypair.generate().publicKey;

  let guardConfigPda: PublicKey;
  let bump: number;

  before(async () => {
    [guardConfigPda, bump] = PublicKey.findProgramAddressSync(
      [Buffer.from("guard_config"), baseMint.toBuffer(), quoteMint.toBuffer()],
      program.programId
    );
  });

  it("1. Successfully initializes guard config with safe parameters", async () => {
    const maxAllowedDeviationBps = new anchor.BN(50); // 0.50%
    const stalenessThresholdSecs = new anchor.BN(30);

    const tx = await program.methods
      .initializeGuardConfig(maxAllowedDeviationBps, stalenessThresholdSecs)
      .accounts({
        guardConfig: guardConfigPda,
        baseMint: baseMint,
        quoteMint: quoteMint,
        admin: admin.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    expect(tx).to.be.a("string");

    const state = await program.account.guardConfig.fetch(guardConfigPda);
    expect(state.admin.toBase58()).to.equal(admin.publicKey.toBase58());
    expect(state.baseMint.toBase58()).to.equal(baseMint.toBase58());
    expect(state.quoteMint.toBase58()).to.equal(quoteMint.toBase58());
    expect(state.maxAllowedDeviationBps.toNumber()).to.equal(50);
    expect(state.stalenessThresholdSecs.toNumber()).to.equal(30);
    expect(state.totalVerifications.toNumber()).to.equal(0);
  });

  it("2. Verifies and records prices within acceptable spread", async () => {
    const now = Math.floor(Date.now() / 1000);
    const venueAPrice = new anchor.BN(175_000_000); // $175.00 scaled by 1e6
    const venueBPrice = new anchor.BN(175_200_000); // $175.20 scaled by 1e6 (~11 bps diff)

    const tx = await program.methods
      .assertAndRecordComparison(venueAPrice, venueBPrice, new anchor.BN(now))
      .accounts({
        guardConfig: guardConfigPda,
        caller: admin.publicKey,
      })
      .rpc();

    expect(tx).to.be.a("string");

    const state = await program.account.guardConfig.fetch(guardConfigPda);
    expect(state.totalVerifications.toNumber()).to.equal(1);
    expect(state.lastVenueAPrice.toNumber()).to.equal(175_000_000);
    expect(state.lastVenueBPrice.toNumber()).to.equal(175_200_000);
  });

  it("3. Fails when price deviation exceeds permissible threshold", async () => {
    const now = Math.floor(Date.now() / 1000);
    const venueAPrice = new anchor.BN(175_000_000); // $175.00
    const venueBPrice = new anchor.BN(180_000_000); // $180.00 (~285 bps diff, limit is 50 bps)

    try {
      await program.methods
        .assertAndRecordComparison(venueAPrice, venueBPrice, new anchor.BN(now))
        .accounts({
          guardConfig: guardConfigPda,
          caller: admin.publicKey,
        })
        .rpc();
      expect.fail("Transaction should have reverted with PriceDivergenceExceeded");
    } catch (err: any) {
      expect(err.error.errorCode.code).to.equal("PriceDivergenceExceeded");
    }
  });

  it("4. Fails when quote timestamp is stale (> 30s)", async () => {
    const staleTime = Math.floor(Date.now() / 1000) - 45; // 45 seconds ago
    const venueAPrice = new anchor.BN(175_000_000);
    const venueBPrice = new anchor.BN(175_050_000);

    try {
      await program.methods
        .assertAndRecordComparison(venueAPrice, venueBPrice, new anchor.BN(staleTime))
        .accounts({
          guardConfig: guardConfigPda,
          caller: admin.publicKey,
        })
        .rpc();
      expect.fail("Transaction should have reverted with PriceQuoteStale");
    } catch (err: any) {
      expect(err.error.errorCode.code).to.equal("PriceQuoteStale");
    }
  });

  it("5. Fails when zero price is submitted", async () => {
    const now = Math.floor(Date.now() / 1000);
    try {
      await program.methods
        .assertAndRecordComparison(new anchor.BN(0), new anchor.BN(175_000_000), new anchor.BN(now))
        .accounts({
          guardConfig: guardConfigPda,
          caller: admin.publicKey,
        })
        .rpc();
      expect.fail("Transaction should have reverted with ZeroPriceProvided");
    } catch (err: any) {
      expect(err.error.errorCode.code).to.equal("ZeroPriceProvided");
    }
  });
});