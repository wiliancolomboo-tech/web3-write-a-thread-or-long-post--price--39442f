import { Connection, Keypair } from "@solana/web3.js";
import * as dotenv from "dotenv";
import { PriceComparator } from "./services/priceComparator";
import { DiscordNotifier } from "./services/discordNotifier";
import { ResendNotifier } from "./services/resendNotifier";

dotenv.config();

const RPC_URL = process.env.SOLANA_RPC_URL || "https://api.mainnet-beta.solana.com";
const DISCORD_WEBHOOK_URL = process.env.DISCORD_WEBHOOK_URL || "";
const RESEND_API_KEY = process.env.RESEND_API_KEY;
const AUDIT_EMAIL = process.env.AUDIT_EMAIL || "ops@solpriceengine.io";

async function main() {
  console.log("=================================================");
  console.log("🚀 SOLANA LIQUIDITY & PRICE COMPARISON ENGINE 🚀");
  console.log("=================================================");
  console.log(`Connecting to RPC: ${RPC_URL}`);

  const connection = new Connection(RPC_URL, "confirmed");
  const comparator = new PriceComparator(connection);
  const discord = new DiscordNotifier(DISCORD_WEBHOOK_URL);
  const resend = new ResendNotifier(RESEND_API_KEY, AUDIT_EMAIL);

  const testQuantitiesSol = [1.0, 10.0, 100.0, 500.0];
  const auditHistory = [];

  for (const solAmount of testQuantitiesSol) {
    console.log(`\nAnalyzing Order Size: ${solAmount} SOL ...`);
    const report = await comparator.compareAll(solAmount);

    console.log(`- Best Venue : ${report.bestVenue}`);
    console.log(`- Worst Venue: ${report.worstVenue}`);
    console.log(`- Spread     : ${report.spreadBps.toFixed(2)} bps`);
    console.log(`- Actionable Arb: ${report.arbOpportunity ? "YES (⚡)" : "NO"}`);

    report.quotes.forEach((q) => {
      console.log(
        `  * [${q.venueName.padEnd(25)}] Net Realized Price: $${q.netRealizedPriceUsdc.toFixed(4)} (Fee: ${q.estimatedFeeSol} SOL)`
      );
    });

    auditHistory.push(report);

    // If spread is notable, trigger live webhook
    if (report.spreadBps > 10.0) {
      await discord.sendAlert(report);
    }
  }

  // Send aggregated telemetry
  console.log("\nDispatching email audit report...");
  await resend.sendDailyAuditEmail(auditHistory);

  console.log("\n[SolPriceEngine] Run cycle completed successfully.");
}

if (require.main === module) {
  main().catch((err) => {
    console.error("Fatal Error running engine:", err);
    process.exit(1);
  });
}