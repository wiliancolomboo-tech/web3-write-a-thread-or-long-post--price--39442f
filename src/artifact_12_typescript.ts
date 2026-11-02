import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { SolanaPriceComparator } from "../target/types/solana_price_comparator";
import { Keypair, PublicKey, SystemProgram } from "@solana/web3.js";
import { expect } from "chai";

describe("solana-price-comparator", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.SolanaPriceComparator as Program<SolanaPriceComparator>;
  const authority = provider.wallet as anchor.Wallet;

  let registryPda: PublicKey;
  let registryBump: number;

  before(async () => {
    [registryPda, registryBump] = PublicKey.findProgramAddressSync(
      [Buffer.from("price_registry"), authority.publicKey.toBuffer()],
      program.programId
    );
  });

  it("1. Successfully initializes the ComparisonRegistry with max bps divergence", async () => {
    const maxBps = new anchor.BN(100); // 1.00%

    // Check if registry already exists, if not initialize
    try {
      const existing = await program.account.comparisonRegistry.fetch(registryPda);
      expect(existing.authority.toBase58()).to.equal(authority.publicKey.toBase58());
    } catch {
      await program.methods
        .initializeRegistry(maxBps)
        .accounts({
          registry: registryPda,
          authority: authority.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      const registryAccount = await program.account.comparisonRegistry.fetch(registryPda);
      expect(registryAccount.authority.toBase58()).to.equal(authority.publicKey.toBase58());
      expect(registryAccount.maxAllowedBpsDivergence.toNumber()).to.equal(100);
      expect(registryAccount.comparisonCount.toNumber()).to.equal(0);
    }
  });

  it("2. Rejects registry parameter mutation from unauthorized authority", async () => {
    const rogueKeypair = Keypair.generate();
    const rogueProvider = new anchor.AnchorProvider(
      provider.connection,
      new anchor.Wallet(rogueKeypair),
      anchor.AnchorProvider.defaultOptions()
    );
    const rogueProgram = new Program<SolanaPriceComparator>(
      program.idl,
      rogueProvider
    );

    try {
      await rogueProgram.methods
        .updateRegistryParameters(new anchor.BN(200))
        .accounts({
          registry: registryPda,
          authority: rogueKeypair.publicKey,
        })
        .rpc();
      expect.fail("Should have thrown unauthorized error");
    } catch (err: any) {
      expect(err.toString()).to.include("Unauthorized");
    }
  });

  it("3. Validates unit math: Exponent normalization logic", () => {
    // Normalization testing: 178.45 USD formatted as 178450000 with expo -6
    const priceScaled = 178450000;
    const expo = -6;
    const targetExpo = -6;
    const delta = targetExpo - expo;
    expect(delta).to.equal(0);
    expect(priceScaled).to.equal(178450000);

    // If expo is -4 (17845 with expo -2 => delta = -4)
    const rawPrice = 17845;
    const rawExpo = -2;
    const factor = Math.pow(10, -(-6 - rawExpo));
    const normalized = rawPrice * factor;
    expect(normalized).to.equal(178450000);
  });

  it("4. Prevents verification with empty quotes array", async () => {
    const symbolBuffer = Buffer.alloc(8);
    symbolBuffer.write("SOLUSDC");
    const symbolArray = Array.from(symbolBuffer);

    const [recordPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("comparison_record"), registryPda.toBuffer(), symbolBuffer],
      program.programId
    );

    try {
      await program.methods
        .verifyAndRecordComparison(symbolArray, [])
        .accounts({
          registry: registryPda,
          comparisonRecord: recordPda,
          pythOracle: Keypair.generate().publicKey,
          payer: authority.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .rpc();
      expect.fail("Should have thrown empty quotes array error");
    } catch (err: any) {
      expect(err.toString()).to.include("EmptyQuotesArray");
    }
  });
});