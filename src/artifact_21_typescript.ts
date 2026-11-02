import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { SolPriceVerifier } from "../target/types/sol_price_verifier";
import { assert, expect } from "chai";
import crypto from "crypto";

describe("sol_price_verifier program tests", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.SolPriceVerifier as Program<SolPriceVerifier>;

  it("Records price comparison when spread is within acceptable bounds", async () => {
    const pairHash = crypto.createHash("sha256").update("SOL/USDC-POOL-TEST").digest();
    const pairHashArray = Array.from(pairHash);

    const priceVenueA = new anchor.BN(150_250_000); // $150.25
    const priceVenueB = new anchor.BN(150_000_000); // $150.00
    const maxSpreadBps = 100; // 1% tolerance (100 bps)

    const [recordPda] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("price_comparison"), pairHash],
      program.programId
    );

    const tx = await program.methods
      .compareAndRecord(
        pairHashArray,
        0, // Orca
        priceVenueA,
        1, // Raydium
        priceVenueB,
        maxSpreadBps
      )
      .accounts({
        record: recordPda,
        authority: provider.wallet.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .rpc();

    expect(tx).to.be.a("string");

    const recordAccount = await program.account.priceComparisonRecord.fetch(recordPda);
    assert.equal(recordAccount.venueAId, 0);
    assert.equal(recordAccount.venueBId, 1);
    assert.equal(recordAccount.priceVenueA.toNumber(), 150_250_000);
    assert.equal(recordAccount.priceVenueB.toNumber(), 150_000_000);
    // Spread: (0.25 / 150.00) * 10,000 = ~16 bps
    assert.equal(recordAccount.spreadBps, 16);
  });

  it("Fails when price divergence exceeds max tolerated spread threshold", async () => {
    const pairHash = crypto.createHash("sha256").update("SOL/USDC-DIVERGENT").digest();
    const pairHashArray = Array.from(pairHash);

    const priceVenueA = new anchor.BN(170_000_000); // $170.00
    const priceVenueB = new anchor.BN(150_000_000); // $150.00
    const maxSpreadBps = 50; // Max allowed 50 bps (~$0.75 diff)

    const [recordPda] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("price_comparison"), pairHash],
      program.programId
    );

    try {
      await program.methods
        .compareAndRecord(
          pairHashArray,
          0,
          priceVenueA,
          1,
          priceVenueB,
          maxSpreadBps
        )
        .accounts({
          record: recordPda,
          authority: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
        })
        .rpc();
      assert.fail("Should have failed with SpreadThresholdExceeded");
    } catch (err: any) {
      expect(err.error.errorCode.code).to.equal("SpreadThresholdExceeded");
    }
  });

  it("Rejects zero prices strictly", async () => {
    const pairHash = crypto.createHash("sha256").update("SOL/USDC-ZERO").digest();
    const pairHashArray = Array.from(pairHash);

    const [recordPda] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("price_comparison"), pairHash],
      program.programId
    );

    try {
      await program.methods
        .compareAndRecord(
          pairHashArray,
          0,
          new anchor.BN(0),
          1,
          new anchor.BN(150_000_000),
          500
        )
        .accounts({
          record: recordPda,
          authority: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
        })
        .rpc();
      assert.fail("Should have failed with ZeroPriceDetected");
    } catch (err: any) {
      expect(err.error.errorCode.code).to.equal("ZeroPriceDetected");
    }
  });
});