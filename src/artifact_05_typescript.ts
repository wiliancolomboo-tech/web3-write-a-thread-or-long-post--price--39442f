import { Connection, PublicKey, Keypair, clusterApiUrl } from '@solana/web3.js';
import axios from 'axios';
import * as dotenv from 'dotenv';

dotenv.config();

interface PriceSample {
  source: 'Pyth' | 'Whirlpool' | 'Jupiter';
  tokenPair: string;
  price: number;
  timestamp: number;
}

interface AlertPayload {
  pair: string;
  pythPrice: number;
  jupPrice: number;
  spreadBps: number;
  status: 'SAFE' | 'WARNING' | 'CRITICAL_ARBITRAGE';
}

class PriceComparisonMonitor {
  private connection: Connection;
  private solUsdPythAccount: PublicKey;
  private alertThresholdBps: number;
  private discordWebhookUrl?: string;
  private slackWebhookUrl?: string;

  constructor() {
    const rpcUrl = process.env.SOLANA_RPC_URL || clusterApiUrl('mainnet-beta');
    this.connection = new Connection(rpcUrl, 'confirmed');

    // SOL/USD Pyth Mainnet Feed Address
    this.solUsdPythAccount = new PublicKey(
      process.env.PYTH_SOL_USD_PUBKEY || 'H6ARHf6YXhGYeQfUzQNGk6rDNnLBQKrenN712K4AQJEG'
    );
    this.alertThresholdBps = parseInt(process.env.ALERT_THRESHOLD_BPS || '25', 10);
    this.discordWebhookUrl = process.env.DISCORD_WEBHOOK_URL;
    this.slackWebhookUrl = process.env.SLACK_WEBHOOK_URL;
  }

  /**
   * Fetches real-time price quote from Jupiter v6 Aggregator API
   */
  async getJupiterQuote(inputMint: string, outputMint: string, amountLamports: number): Promise<number> {
    try {
      const url = `https://quote-api.jup.ag/v6/quote?inputMint=${inputMint}&outputMint=${outputMint}&amount=${amountLamports}&slippageBps=50`;
      const response = await axios.get(url, { timeout: 3500 });
      const outAmount = Number(response.data.outAmount);
      // Normalized for SOL (9 decimals) to USDC (6 decimals)
      const executionPrice = (outAmount / 1e6) / (amountLamports / 1e9);
      return executionPrice;
    } catch (err: any) {
      console.error(`[Jupiter Quote Error]: ${err.message}`);
      return 0;
    }
  }

  /**
   * Reads raw bytes from Solana account and extracts Pyth price data
   */
  async getPythPrice(feedAccount: PublicKey): Promise<number> {
    try {
      const accountInfo = await this.connection.getAccountInfo(feedAccount);
      if (!accountInfo) throw new Error('Failed to find Pyth Account Info');

      // Pyth V2 price deserialization offset for price (i64 at offset 208) and expo (i32 at offset 200)
      const data = accountInfo.data;
      const priceOffset = 208;
      const expoOffset = 200;

      const price = data.readBigInt64LE(priceOffset);
      const expo = data.readInt32LE(expoOffset);

      return Number(price) * Math.pow(10, expo);
    } catch (err: any) {
      console.error(`[Pyth Feed Error]: ${err.message}`);
      return 0;
    }
  }

  /**
   * Computes discrepancy and triggers enterprise webhooks
   */
  async evaluateComparison(): Promise<void> {
    const solMint = 'So11111111111111111111111111111111111111112';
    const usdcMint = 'EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v';
    const testAmountLamports = 10 * 1e9; // 10 SOL sample trade

    const [pythPrice, jupPrice] = await Promise.all([
      this.getPythPrice(this.solUsdPythAccount),
      this.getJupiterQuote(solMint, usdcMint, testAmountLamports),
    ]);

    if (pythPrice <= 0 || jupPrice <= 0) {
      console.warn('[Telemetry] Skipping iteration due to zero or failed quote retrieval');
      return;
    }

    const spreadAbs = Math.abs(pythPrice - jupPrice);
    const spreadBps = (spreadAbs / pythPrice) * 10000;

    const payload: AlertPayload = {
      pair: 'SOL/USDC',
      pythPrice: parseFloat(pythPrice.toFixed(4)),
      jupPrice: parseFloat(jupPrice.toFixed(4)),
      spreadBps: parseFloat(spreadBps.toFixed(2)),
      status: spreadBps > this.alertThresholdBps ? 'CRITICAL_ARBITRAGE' : 'SAFE',
    };

    console.log(
      `[${new Date().toISOString()}] ${payload.pair} | Pyth: $${payload.pythPrice} | Jupiter: $${payload.jupPrice} | Spread: ${payload.spreadBps} bps | Status: ${payload.status}`
    );

    if (payload.status === 'CRITICAL_ARBITRAGE') {
      await this.dispatchAlert(payload);
    }
  }

  /**
   * Dispatches alerts to Discord and Slack
   */
  private async dispatchAlert(payload: AlertPayload): Promise<void> {
    const message = {
      content: `🚨 **Price Discrepancy Alert: ${payload.pair}**\n` +
               `• **Pyth Oracle Price**: $${payload.pythPrice}\n` +
               `• **Jupiter Swap Price**: $${payload.jupPrice}\n` +
               `• **Spread**: ${payload.spreadBps} bps (Threshold: ${this.alertThresholdBps} bps)\n` +
               `• **Recommended Action**: Trigger institutional cross-venue arbitrage rebalance or delay liquidation.`
    };

    if (this.discordWebhookUrl) {
      try {
        await axios.post(this.discordWebhookUrl, message);
      } catch (err: any) {
        console.error(`[Discord Webhook Delivery Error]: ${err.message}`);
      }
    }

    if (this.slackWebhookUrl) {
      try {
        await axios.post(this.slackWebhookUrl, { text: message.content });
      } catch (err: any) {
        console.error(`[Slack Webhook Delivery Error]: ${err.message}`);
      }
    }
  }

  public async startLoop(intervalMs: number = 3000): Promise<void> {
    console.log(`Starting Solana Enterprise Price Comparison Monitor [Poll Interval: ${intervalMs}ms]...`);
    const run = async () => {
      try {
        await this.evaluateComparison();
      } catch (err: any) {
        console.error(`Error in loop cycle: ${err.message}`);
      }
      setTimeout(run, intervalMs);
    };
    run();
  }
}

// Entrypoint
if (require.main === module) {
  const monitor = new PriceComparisonMonitor();
  monitor.startLoop(3000);
}