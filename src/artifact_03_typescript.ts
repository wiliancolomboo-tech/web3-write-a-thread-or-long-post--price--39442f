// Path: /src/index.ts
import axios from "axios";
import { Connection, PublicKey } from "@solana/web3.js";
import dotenv from "dotenv";

dotenv.config();

export interface PricePoint {
  venue: string;
  inAmount: string;
  outAmount: string;
  effectivePrice: number;
  priceImpactPct?: number;
  timestampMs: number;
}

export interface ComparisonReport {
  timestamp: string;
  inputToken: string;
  outputToken: string;
  amountIn: number;
  bestVenue: string;
  worstVenue: string;
  spreadBps: number;
  quotes: PricePoint[];
}

export class SolanaPriceSentinel {
  private connection: Connection;
  private jupiterEndpoint: string;
  private discordWebhookUrl?: string;
  private slackWebhookUrl?: string;
  private maxAllowedSpreadBps: number;

  constructor() {
    const rpcUrl = process.env.SOLANA_RPC_URL || "https://api.mainnet-beta.solana.com";
    this.connection = new Connection(rpcUrl, "confirmed");
    this.jupiterEndpoint = process.env.JUPITER_API_URL || "https://quote-api.jup.ag/v6";
    this.discordWebhookUrl = process.env.DISCORD_WEBHOOK_URL;
    this.slackWebhookUrl = process.env.SLACK_WEBHOOK_URL;
    this.maxAllowedSpreadBps = parseInt(process.env.MAX_SPREAD_BPS || "35", 10);
  }

  /**
   * Fetches deterministic Jupiter Aggregator V6 Quote
   */
  public async fetchJupiterQuote(
    inputMint: string,
    outputMint: string,
    amountRaw: bigint,
    slippageBps: number = 50
  ): Promise<PricePoint | null> {
    try {
      const url = `${this.jupiterEndpoint}/quote?inputMint=${inputMint}&outputMint=${outputMint}&amount=${amountRaw.toString()}&slippageBps=${slippageBps}`;
      const res = await axios.get(url, { timeout: 4500 });
      if (!res.data || !res.data.outAmount) return null;

      const outAmount = res.data.outAmount;
      const effectivePrice = Number(outAmount) / Number(amountRaw);

      return {
        venue: "Jupiter v6 Aggregator",
        inAmount: amountRaw.toString(),
        outAmount: outAmount.toString(),
        effectivePrice,
        priceImpactPct: parseFloat(res.data.priceImpactPct || "0.0"),
        timestampMs: Date.now(),
      };
    } catch (err: any) {
      console.error(`[Jupiter Quote Fetch Error]: ${err.message}`);
      return null;
    }
  }

  /**
   * Fetches Raydium Isolated Route Quote via Jupiter venue filter
   */
  public async fetchRaydiumQuote(
    inputMint: string,
    outputMint: string,
    amountRaw: bigint
  ): Promise<PricePoint | null> {
    try {
      const url = `${this.jupiterEndpoint}/quote?inputMint=${inputMint}&outputMint=${outputMint}&amount=${amountRaw.toString()}&dexes=Raydium,Raydium%20CPMM,Raydium%20CLMM`;
      const res = await axios.get(url, { timeout: 4500 });
      if (!res.data || !res.data.outAmount) return null;

      return {
        venue: "Raydium (Isolated Pool)",
        inAmount: amountRaw.toString(),
        outAmount: res.data.outAmount.toString(),
        effectivePrice: Number(res.data.outAmount) / Number(amountRaw),
        priceImpactPct: parseFloat(res.data.priceImpactPct || "0.0"),
        timestampMs: Date.now(),
      };
    } catch (err: any) {
      console.error(`[Raydium Quote Fetch Error]: ${err.message}`);
      return null;
    }
  }

  /**
   * Fetches Orca Whirlpool Isolated Route Quote via Jupiter venue filter
   */
  public async fetchOrcaQuote(
    inputMint: string,
    outputMint: string,
    amountRaw: bigint
  ): Promise<PricePoint | null> {
    try {
      const url = `${this.jupiterEndpoint}/quote?inputMint=${inputMint}&outputMint=${outputMint}&amount=${amountRaw.toString()}&dexes=Whirlpool`;
      const res = await axios.get(url, { timeout: 4500 });
      if (!res.data || !res.data.outAmount) return null;

      return {
        venue: "Orca Whirlpool (CLMM)",
        inAmount: amountRaw.toString(),
        outAmount: res.data.outAmount.toString(),
        effectivePrice: Number(res.data.outAmount) / Number(amountRaw),
        priceImpactPct: parseFloat(res.data.priceImpactPct || "0.0"),
        timestampMs: Date.now(),
      };
    } catch (err: any) {
      console.error(`[Orca Quote Fetch Error]: ${err.message}`);
      return null;
    }
  }

  /**
   * Compares all venues and generates disparity telemetry
   */
  public async comparePrices(
    inputMint: string,
    outputMint: string,
    amountRaw: bigint,
    inputDecimals: number,
    outputDecimals: number
  ): Promise<ComparisonReport | null> {
    const [jup, ray, orca] = await Promise.all([
      this.fetchJupiterQuote(inputMint, outputMint, amountRaw),
      this.fetchRaydiumQuote(inputMint, outputMint, amountRaw),
      this.fetchOrcaQuote(inputMint, outputMint, amountRaw),
    ]);

    const quotes: PricePoint[] = [jup, ray, orca].filter(
      (q): q is PricePoint => q !== null
    );

    if (quotes.length < 2) {
      console.warn("Insufficient quotes collected to execute comparative analysis.");
      return null;
    }

    // Sort by outAmount descending (highest out is best for trader)
    quotes.sort((a, b) => Number(BigInt(b.outAmount) - BigInt(a.outAmount)));

    const best = quotes[0];
    const worst = quotes[quotes.length - 1];

    const diff = Number(BigInt(best.outAmount) - BigInt(worst.outAmount));
    const spreadBps = (diff / Number(worst.outAmount)) * 10000;

    const report: ComparisonReport = {
      timestamp: new Date().toISOString(),
      inputToken: inputMint,
      outputToken: outputMint,
      amountIn: Number(amountRaw) / 10 ** inputDecimals,
      bestVenue: best.venue,
      worstVenue: worst.venue,
      spreadBps: Math.round(spreadBps * 100) / 100,
      quotes,
    };

    if (report.spreadBps >= this.maxAllowedSpreadBps) {
      await this.dispatchEnterpriseAlert(report);
    }

    return report;
  }

  /**
   * Enterprise Dispatches (Discord / Slack)
   */
  private async dispatchEnterpriseAlert(report: ComparisonReport): Promise<void> {
    const message = {
      title: "🚨 Solana Execution Price Disparity Alert",
      color: 16711680,
      fields: [
        { name: "Spread Divergence", value: `**${report.spreadBps} bps**`, inline: true },
        { name: "Optimal Route", value: report.bestVenue, inline: true },
        { name: "Suboptimal Route", value: report.worstVenue, inline: true },
        {
          name: "Quotes Breakdown",
          value: report.quotes
            .map((q) => `• **${q.venue}**: ${q.outAmount} out (Impact: ${q.priceImpactPct}%)`)
            .join("\n"),
        },
      ],
      timestamp: report.timestamp,
    };

    if (this.discordWebhookUrl) {
      try {
        await axios.post(this.discordWebhookUrl, { embeds: [message] });
      } catch (err: any) {
        console.error(`Failed to dispatch Discord alert: ${err.message}`);
      }
    }

    if (this.slackWebhookUrl) {
      try {
        await axios.post(this.slackWebhookUrl, {
          text: `*🚨 Solana Price Disparity Detected: ${report.spreadBps} bps spread between ${report.bestVenue} and ${report.worstVenue}*`,
        });
      } catch (err: any) {
        console.error(`Failed to dispatch Slack alert: ${err.message}`);
      }
    }
  }

  /**
   * Continuous Monitoring Loop
   */
  public async runDaemon(intervalMs: number = 10000): Promise<void> {
    console.log("⚡ Starting Solana Price Sentinel Daemon...");
    // SOL -> USDC mint addresses on Mainnet-Beta
    const SOL_MINT = "So11111111111111111111111111111111111111112";
    const USDC_MINT = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
    const TRADE_SIZE_RAW = BigInt(10_000_000_000); // 10 SOL

    setInterval(async () => {
      try {
        const report = await this.comparePrices(
          SOL_MINT,
          USDC_MINT,
          TRADE_SIZE_RAW,
          9,
          6
        );
        if (report) {
          console.log(
            `[${report.timestamp}] Best: ${report.bestVenue} | Spread: ${report.spreadBps} bps`
          );
        }
      } catch (e: any) {
        console.error(`Loop iteration failure: ${e.message}`);
      }
    }, intervalMs);
  }
}

// Immediate Execution on Node entry point
if (require.main === module) {
  const sentinel = new SolanaPriceSentinel();
  sentinel.runDaemon(10000);
}