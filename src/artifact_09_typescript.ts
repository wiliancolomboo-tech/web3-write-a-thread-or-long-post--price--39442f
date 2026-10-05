import { expect } from "chai";
import { PublicKey } from "@solana/web3.js";
import BN from "bn.js";
import { SolPrismQuoter } from "../src/price-engine";

describe("SolPrism Comprehensive Price Comparison Engine Tests", () => {
  const dummyRpc = "https://api.mainnet-beta.solana.com";
  const quoter = new SolPrismQuoter(dummyRpc);

  const SOL_MINT = new PublicKey("So11111111111111111111111111111111111111112");
  const USDC_MINT = new PublicKey("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");

  it("should evaluate quotes across all 3 venues with positive outputs", async () => {
    const amountIn = new BN(5 * 1e9); // 5 SOL
    const result = await quoter.comparePrices(SOL_MINT, USDC_MINT, amountIn);

    expect(result.quotes).to.have.lengthOf(3);
    result.quotes.forEach((q) => {
      expect(q.outAmount.toNumber()).to.be.greaterThan(0);
      expect(q.effectivePrice).to.be.greaterThan(100); // SOL price floor
      expect(q.executionLatencyMs).to.be.lessThan(500); // Under 500ms
    });
  });

  it("should select the venue with mathematically maximal return as optimalVenue", async () => {
    const amountIn = new BN(10 * 1e9);
    const result = await quoter.comparePrices(SOL_MINT, USDC_MINT, amountIn);

    const maxOut = Math.max(...result.quotes.map((q) => q.outAmount.toNumber()));
    expect(result.optimalVenue.outAmount.toNumber()).to.equal(maxOut);
  });

  it("should compute correct non-negative divergence basis points", async () => {
    const amountIn = new BN(50 * 1e9);
    const result = await quoter.comparePrices(SOL_MINT, USDC_MINT, amountIn);

    expect(result.divergenceBps).to.be.greaterThanOrEqual(0);
    expect(result.divergenceBps).to.be.lessThan(5000); // Sensible sanity bound (< 50%)
  });

  it("should accurately capture increasing price impact with larger trade sizes", async () => {
    const smallAmount = new BN(1 * 1e9);
    const largeAmount = new BN(500 * 1e9);

    const smallResult = await quoter.comparePrices(SOL_MINT, USDC_MINT, smallAmount);
    const largeResult = await quoter.comparePrices(SOL_MINT, USDC_MINT, largeAmount);

    const orcaSmall = smallResult.quotes.find((q) => q.venueName === "Orca Whirlpool")!;
    const orcaLarge = largeResult.quotes.find((q) => q.venueName === "Orca Whirlpool")!;

    expect(orcaLarge.priceImpactPct).to.be.greaterThan(orcaSmall.priceImpactPct);
  });
});