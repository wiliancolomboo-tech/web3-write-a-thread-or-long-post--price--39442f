import { Connection, Keypair, PublicKey } from '@solana/web3.js';
import * as dotenv from 'dotenv';
import { DexQuote, ComparisonEvaluation } from './types';
import { AlertManager } from './alertManager';

dotenv.config();

export class SolanaPriceComparatorAgent {
  private connection: Connection;
  private alertManager: AlertManager;
  private maxAllowedBps: number;

  constructor() {
    const rpcUrl = process.env.SOLANA_RPC_URL || 'https://api.mainnet-beta.solana.com';
    this.connection = new Connection(rpcUrl, 'confirmed');
    this.alertManager = new AlertManager();
    this.maxAllowedBps = Number(process.env.MAX_ALLOWED_BPS || '50');
  }

  /**
   * Fetch spot & virtual tick quotes across major Solana DEX liquidity venues.
   * Simulates high-precision orderbook/pool state fetching.
   */
  public async fetchCandidateQuotes(pair: string): Promise<DexQuote[]> {
    // In production, queries Raydium CLMM pool state, Orca Whirlpool ticks, and Phoenix BBO accounts.
    // Simulated realistic market condition quotes:
    const basePrices: Record<string, number> = {
      'SOL/USDC': 178.45,
      'JTO/USDC': 2.82,
      'JUP/USDC': 1.05,
    };

    const mid = basePrices[pair] || 100.0;

    return [
      {
        dexId: 1,
        dexName: 'Raydium CLMM',
        priceScaled: Math.round((mid + 0.04) * 1_000_000),
        expo: -6,
        normalizedPrice: mid + 0.04,
        volumeDepthUsd: 1_250_000,
      },
      {
        dexId: 2,
        dexName: 'Orca Whirlpool',
        priceScaled: Math.round((mid + 0.09) * 1_000_000),
        expo: -6,
        normalizedPrice: mid + 0.09,
        volumeDepthUsd: 2_400_000,
      },
      {
        dexId: 3,
        dexName: 'Phoenix L3 Orderbook',
        priceScaled: Math.round((mid + 0.02) * 1_000_000),
        expo: -6,
        normalizedPrice: mid + 0.02,
        volumeDepthUsd: 890_000,
      },
      {
        dexId: 4,
        dexName: 'Meteora DLMM',
        priceScaled: Math.round((mid + 0.06) * 1_000_000),
        expo: -6,
        normalizedPrice: mid + 0.06,
        volumeDepthUsd: 650_000,
      },
    ];
  }

  public async fetchPythBenchmark(pair: string): Promise<number> {
    // Pyth benchmark price with negligible divergence
    const benchmarkTable: Record<string, number> = {
      'SOL/USDC': 178.46,
      'JTO/USDC': 2.821,
      'JUP/USDC': 1.0505,
    };
    return benchmarkTable[pair] || 100.0;
  }

  public evaluateQuotes(
    symbol: string,
    quotes: DexQuote[],
    oracleBenchmark: number
  ): ComparisonEvaluation {
    if (!quotes.length) {
      throw new Error('Quotes array cannot be empty');
    }

    const sorted = [...quotes].sort((a, b) => b.normalizedPrice - a.normalizedPrice);
    const bestVenue = sorted[0];
    const runnerUpVenue = sorted[1] || sorted[0];

    const oracleDivergenceBps =
      (Math.abs(bestVenue.normalizedPrice - oracleBenchmark) / oracleBenchmark) * 10_000;
    const spreadBps =
      ((bestVenue.normalizedPrice - runnerUpVenue.normalizedPrice) / runnerUpVenue.normalizedPrice) *
      10_000;

    return {
      symbol,
      bestVenue,
      runnerUpVenue,
      oraclePrice: oracleBenchmark,
      oracleDivergenceBps,
      spreadBps,
      timestamp: Date.now(),
    };
  }

  public async runSurveillanceLoop(symbol: string): Promise<ComparisonEvaluation> {
    console.log(`[Surveillance] Initiating price comparison tick for ${symbol}...`);

    const [quotes, oraclePrice] = await Promise.all([
      this.fetchCandidateQuotes(symbol),
      this.fetchPythBenchmark(symbol),
    ]);

    const evaluation = this.evaluateQuotes(symbol, quotes, oraclePrice);

    console.log(
      `[Result] Best Venue: ${evaluation.bestVenue.dexName} @ $${evaluation.bestVenue.normalizedPrice.toFixed(
        4
      )} | Pyth Oracle: $${evaluation.oraclePrice.toFixed(4)} | Divergence: ${evaluation.oracleDivergenceBps.toFixed(
        2
      )} bps`
    );

    if (evaluation.oracleDivergenceBps > this.maxAllowedBps) {
      console.warn(
        `[WARNING] Divergence ${evaluation.oracleDivergenceBps.toFixed(
          2
        )} bps exceeds threshold ${this.maxAllowedBps} bps! Rejection triggered.`
      );
    } else {
      console.log(`[PASS] Price divergence within acceptable tolerance.`);
    }

    await this.alertManager.dispatchExecutionAlert(evaluation);
    return evaluation;
  }
}

// CLI Execution Entry Point
if (require.main === module) {
  const agent = new SolanaPriceComparatorAgent();
  agent
    .runSurveillanceLoop('SOL/USDC')
    .then(() => {
      console.log('[Engine] Surveillance task completed successfully.');
      process.exit(0);
    })
    .catch((err) => {
      console.error('[Engine Fatal]', err);
      process.exit(1);
    });
}