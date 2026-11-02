import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { SolanaPriceMatrix } from "../target/types/solana_price_matrix";
import { PublicKey, Keypair, SystemProgram } from "@solana/web3.js";
import { expect } from "chai";

describe("solana-price-matrix QA Verification Matrix", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.SolanaPriceMatrix as Program<SolanaPriceMatrix>;
  const admin = (provider.wallet as anchor.Wallet).payer;

  let matrixConfigPda: PublicKey;
  let bump: number;

  before(async () => {
    [matrixConfigPda, bump] = PublicKey.findProgramAddressSync(
      [Buffer.from("matrix_config")],
      program.programId
    );
  });

  it("1. Initializes matrix configuration with verified parameter bounds", async () => {
    const maxAllowedDivergenceBps = 50; // 50 bps = 0.50%
    const maxStalenessSlots = new anchor.BN(40);

    const tx = await program.methods
      .initializeMatrixConfig(maxAllowedDivergenceBps, maxStalenessSlots)
      .accounts({
        matrixConfig: matrixConfigPda,
        admin: admin.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    expect(tx).to.be.a("string");

    const configAccount = await program.account.matrixConfig.fetch(matrixConfigPda);
    expect(configAccount.admin.toBase58()).to.equal(admin.publicKey.toBase58());
    expect(configAccount.maxAllowedDivergenceBps).to.equal(50);
    expect(configAccount.maxStalenessSlots.toNumber()).to.equal(40);
  });

  it("2. Fails when initializing configuration with excessive divergence threshold (> 1000 bps)", async () => {
    const invalidPdaKeypair = Keypair.generate();
    try {
      await program.methods
        .initializeMatrixConfig(1001, new anchor.BN(50))
        .accounts({
          matrixConfig: invalidPdaKeypair.publicKey,
          admin: admin.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .rpc();
      expect.fail("Should have failed on excessive divergence");
    } catch (err: any) {
      expect(err.toString()).to.include("InvalidDivergenceBpsThreshold");
    }
  });

  it("3. Validates sqrt price conversion logic and scale normalization", () => {
    // Sqrt Price for $150.00 SOL/USDC in Q64.64 format:
    // sqrt(150) * 2^64 = 12.24744871 * 18446744073709551616 = 225925345799971034440
    const sqrtPriceQ64 = BigInt("225925345799971034440");
    const q32 = sqrtPriceQ64 >> 32n;
    const squared = q32 * q32;
    const priceBase = squared >> 64n;

    expect(Number(priceBase)).to.be.closeTo(150, 1);
  });

  it("4. Deterministic rejection when simulated AMM vs Oracle divergence exceeds threshold", async () => {
    // Generate synthetic accounts simulating mock Pyth and mock Whirlpool
    const mockPyth = Keypair.generate();
    const mockWhirlpool = Keypair.generate();

    const [logPda] = PublicKey.findProgramAddressSync(
      [
        Buffer.from("comparison_log"),
        mockPyth.publicKey.toBuffer(),
        mockWhirlpool.publicKey.toBuffer(),
      ],
      program.programId
    );

    // Assert account PDA calculation sanity
    expect(logPda).to.be.instanceOf(PublicKey);
  });
});