import { expect } from "chai";
import { SolanaPriceComparator, VenueQuote } from "../src/comparator-engine";

describe("Solana Price Comparator Engine - Test Matrix", () => {
  const mockRpcUrl = "https://api.mainnet-beta.solana.com";
  let engine: SolanaPriceComparator;

  before(() => {
    engine = new SolanaPriceComparator(mockRpcUrl);
  });

  describe("Deterministic Quote Evaluation", () => {
    it("should calculate Orca Whirlpool CLMM quote with non-zero proceeds and bounded slippage", async () => {
      const inputLamports = BigInt(10 * 1e9); // 10 SOL
      const quote: VenueQuote = await engine.getOrcaQuote(inputLamports);

      expect(quote.venueId).to.equal(1);
      expect(quote.venueName).to.equal("Orca Whirlpool");
      expect(Number(quote.outputAmountUnits)).to.be.greaterThan(0);
      expect(quote.priceImpactBps).to.be.lessThan(500); // Under 5% for 10 SOL
      expect(quote.netProceedsUnits).to.be.lessThan(quote.outputAmountUnits); // Fees deducted
    });

    it("should calculate Raydium CPMM quote with higher price impact under larger volume", async () => {
      const smallSize = BigInt(1 * 1e9);
      const largeSize = BigInt(1000 * 1e9);

      const smallQuote = await engine.getRaydiumQuote(smallSize);
      const largeQuote = await engine.getRaydiumQuote(largeSize);

      expect(largeQuote.priceImpactBps).to.be.greaterThan(smallQuote.priceImpactBps);
    });

    it("should accurately amortize compute unit cost in Phoenix CLOB", async () => {
      const inputLamports = BigInt(5 * 1e9);
      const quote = await engine.getPhoenixQuote(inputLamports);

      expect(quote.estimatedComputeUnits).to.equal(65_000);
      expect(quote.netProceedsUnits).to.be.greaterThan(0n);
    });
  });

  describe("Best Execution Routing Logic", () => {
    it("should select the venue with highest net proceeds (inclusive of fees)", async () => {
      const report = await engine.compareVenues(100);

      expect(report.quotes.length).to.equal(3);
      expect(report.bestVenue).to.deep.equal(report.quotes[0]);

      // Ensure quotes are sorted strictly descending by net proceeds
      for (let i = 0; i < report.quotes.length - 1; i++) {
        expect(report.quotes[i].netProceedsUnits >= report.quotes[i + 1].netProceedsUnits).to.be.true;
      }
    });

    it("should detect non-negative spread divergence", async () => {
      const report = await engine.compareVenues(25);
      expect(report.divergencePercentage).to.be.greaterThanOrEqual(0);
    });
  });

  describe("Edge Case & Safety Handling", () => {
    it("should handle microscopic micro-trades (0.001 SOL) without integer underflow", async () => {
      const report = await engine.compareVenues(0.001);
      expect(report.bestVenue.outputAmountUnits).to.be.greaterThan(0n);
    });

    it("should gracefully handle large whales without throwing NaN or infinity", async () => {
      const report = await engine.compareVenues(100_000); // 100,000 SOL
      expect(Number.isFinite(report.bestVenue.executionPrice)).to.be.true;
      expect(report.divergencePercentage).to.be.greaterThan(0);
    });
  });
});