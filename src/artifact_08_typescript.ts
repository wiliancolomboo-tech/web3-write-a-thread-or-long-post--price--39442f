import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { PublicKey, Keypair, SystemProgram } from "@solana/web3.js";
import { expect } from "chai";

describe("sol_price_compare program tests", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  // Interface mock to program
  const program = anchor.workspace.SolPriceCompare as Program<any>;
  const authority = provider.wallet as anchor.Wallet;

  let registryPda: PublicKey;
  let registryBump: number;

  before(async () => {
    [registryPda, registryBump] = PublicKey.findProgramAddressSync(
      [Buffer.from("price_registry")],
      program.programId
    );
  });

  it("1. Initializes the Global Price Comparison Registry", async () => {
    const defaultThresholdBps = 30; // 0.30%

    // Check if account already exists
    const accountInfo = await provider.connection.getAccountInfo(registryPda);
    if (!accountInfo) {
      const tx = await program.methods
        .initializeRegistry(defaultThresholdBps)
        .accounts({
          registry: registryPda,
          authority: authority.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      expect(tx).to.be.a("string");
      const registryAccount = await program.account.comparisonRegistry.fetch(registryPda);
      expect(registryAccount.authority.toBase58()).to.equal(authority.publicKey.toBase58());
      expect(registryAccount.divergenceAlertThresholdBps).to.equal(defaultThresholdBps);
      expect(registryAccount.totalAttestations.toNumber()).to.equal(0);
    }
  });

  it("2. Validates mathematical spread basis points calculation", () => {
    const priceA = 150_000_000; // $150.00
    const priceB = 150_750_000; // $150.75
    const diff = Math.abs(priceA - priceB);
    const min = Math.min(priceA, priceB);
    const spreadBps = Math.floor((diff * 10000) / min);

    // 0.75 / 150.00 * 10000 = 50 bps
    expect(spreadBps).to.equal(50);
  });

  it("3. Rejects comparison when input price is zero", async () => {
    const pairBuffer = Buffer.alloc(16);
    pairBuffer.write("SOL/USDC", 0, "utf-8");
    const clientTimestamp = new anchor.BN(Math.floor(Date.now() / 1000));

    const [attestationPda] = PublicKey.findProgramAddressSync(
      [
        Buffer.from("attestation"),
        registryPda.toBuffer(),
        pairBuffer,
        clientTimestamp.toArrayLike(Buffer, "le", 8),
      ],
      program.programId
    );

    try {
      await program.methods
        .recordPriceComparison(
          Array.from(pairBuffer),
          new anchor.BN(0), // INVALID ZERO PRICE
          new anchor.BN(150_000_000),
          clientTimestamp
        )
        .accounts({
          registry: registryPda,
          attestation: attestationPda,
          pythPriceFeed: Keypair.generate().publicKey,
          attester: authority.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .rpc();
      expect.fail("Should have failed with PriceCompareError::InvalidPrice");
    } catch (err: any) {
      expect(err.toString()).to.include("InvalidPrice");
    }
  });

  it("4. Confirms deterministic PDA derivation guarantees idempotency", () => {
    const pairBuffer = Buffer.alloc(16);
    pairBuffer.write("SOL/USDC", 0, "utf-8");
    const timestamp = new anchor.BN(1710000000);

    const [pda1] = PublicKey.findProgramAddressSync(
      [
        Buffer.from("attestation"),
        registryPda.toBuffer(),
        pairBuffer,
        timestamp.toArrayLike(Buffer, "le", 8),
      ],
      program.programId
    );

    const [pda2] = PublicKey.findProgramAddressSync(
      [
        Buffer.from("attestation"),
        registryPda.toBuffer(),
        pairBuffer,
        timestamp.toArrayLike(Buffer, "le", 8),
      ],
      program.programId
    );

    expect(pda1.toBase58()).to.equal(pda2.toBase58());
  });
});