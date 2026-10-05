import { PublicKey } from "@solana/web3.js";
import BN from "bn.js";
import * as dotenv from "dotenv";
import { SolPrismQuoter } from "./price-engine";
import { EnterpriseDispatcher } from "./enterprise-dispatcher";

dotenv.config();

const RPC_URL = process.env.SOLANA_RPC_URL || "https://api.mainnet-beta.solana.com";
const WS_DISCORD = process.env.DISCORD_WEBHOOK_URL;
const RESEND_KEY = process.env.RESEND_API_KEY;
const ALERT_EMAIL = process.env.ALERT_EMAIL_RECIPIENT;

const SOL_MINT = new PublicKey("So11111111111111111111111111111111111111112");
const USDC_MINT = new PublicKey("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");

async function main() {
  console.log("===============================================================");
  console.log("   SOLPRISM: PRODUCTION PRICE COMPARISON & ROUTING ENGINE      ");
  console.log("===============================================================");

  const quoter = new SolPrismQuoter(RPC_URL);
  const dispatcher = new EnterpriseDispatcher({
    discordWebhookUrl: WS_DISCORD,
    resendApiKey: RESEND_KEY,
    alertEmailRecipient: ALERT_EMAIL,
  });

  const tradeSizes = [
    new BN(1 * 1e9),    // 1 SOL
    new BN(10 * 1e9),   // 10 SOL
    new BN(100 * 1e9),  // 100 SOL
    new BN(1000 * 1e9), // 1,000 SOL (Whale size)
  ];

  console.log(`Executing multi-venue price comparison against Live DEX state...\n`);

  for (const size of tradeSizes) {
    const formattedIn = size.toNumber() / 1e9;
    console.log(`---------------------------------------------------------------`);
    console.log(`[ANALYSIS] Input Swap: ${formattedIn} SOL -> USDC`);

    const result = await quoter.comparePrices(SOL_MINT, USDC_MINT, size);

    result.quotes.forEach((q) => {
      console.log(
        ` > [${q.venueName.padEnd(18)}] Out: ${(q.outAmount.toNumber() / 1e6).toFixed(4)} USDC | ` +
        `Rate: $${q.effectivePrice.toFixed(4)} | ` +
        `Slip: ${q.priceImpactPct.toFixed(3)}% | ` +
        `Latency: ${q.executionLatencyMs}ms`
      );
    });

    console.log(` > Winner: ${result.optimalVenue.venueName} | Discrepancy: ${result.divergenceBps} bps`);

    // Dispatch telemetry
    await dispatcher.dispatchToDiscord(result);

    // If divergence exceeds 25 bps on whale orders, trigger executive dispatch
    if (result.divergenceBps > 25 && formattedIn >= 100) {
      console.log(`[ALERT] High divergence detected (${result.divergenceBps} bps). Triggering Resend Notification...`);
      await dispatcher.dispatchDivergenceAlertEmail(result);
    }
  }

  console.log("\n[SUCCESS] Price comparison cycle completed deterministically.");
}

if (require.main === module) {
  main().catch((err) => {
    console.error("Fatal exception in engine runner:", err);
    process.exit(1);
  });
}