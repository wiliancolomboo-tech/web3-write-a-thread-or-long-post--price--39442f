import { PriceComparator } from "../client/src/services/priceComparator";
import { Connection } from "@solana/web3.js";
import { VenueId } from "../client/src/types";

describe("SolPriceEngine Unit Tests", () => {
  let mockConnection: Connection;
  let comparator: PriceComparator;

  beforeEach(() => {
    mockConnection = {
      getSlot: jest.fn().mockResolvedValue(275849120),
    } as unknown as Connection;
    comparator = new PriceComparator(mockConnection);
  });

  test("Orca Whirlpool quote applies lower price impact than Raydium for medium orders", async () => {
    const basePrice = 150.0;
    const sizeSol = 100.0;

    const orca = await comparator.getOrcaQuote(sizeSol, basePrice);
    const raydium = await comparator.getRaydiumQuote(sizeSol, basePrice);

    expect(orca.effectivePriceUsdcPerSol).toBeGreaterThan(0);
    expect(raydium.effectivePriceUsdcPerSol).toBeGreaterThan(0);
    // Concentrated liquidity tick math yields better price retention
    expect(orca.netRealizedPriceUsdc).toBeGreaterThan(raydium.netRealizedPriceUsdc);
  });

  test("Phoenix CLOB orderbook execution has lowest slippage", async () => {
    const basePrice = 150.0;
    const sizeSol = 10.0;

    const phoenix = await comparator.getPhoenixQuote(sizeSol, basePrice);
    expect(phoenix.slippageBps).toBe(2);
    expect(phoenix.venue).toEqual(VenueId.PHOENIX);
  });

  test("Spread computation identifies arbitrage opportunity when gap > 15 bps", async () => {
    const report = await comparator.compareAll(250.0);

    expect(report.quotes.length).toBe(4);
    expect(report.pair).toBe("SOL/USDC");
    expect(typeof report.spreadBps).toBe("number");
    expect(report.spreadBps).toBeGreaterThanOrEqual(0);

    if (report.spreadBps >= 15.0) {
      expect(report.arbOpportunity).toBe(true);
    } else {
      expect(report.arbOpportunity).toBe(false);
    }
  });

  test("Deterministic fee netting strictly reduces gross payout", async () => {
    const basePrice = 140.0;
    const quote = await comparator.getOrcaQuote(1.0, basePrice);
    expect(quote.netRealizedPriceUsdc).toBeLessThan(quote.outAmountUsdc);
  });
});