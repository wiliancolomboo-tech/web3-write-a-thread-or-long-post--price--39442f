import axios from "axios";
import { ComparisonReport } from "../types";

export class DiscordNotifier {
  private webhookUrl: string;

  constructor(webhookUrl: string) {
    this.webhookUrl = webhookUrl;
  }

  public async sendAlert(report: ComparisonReport): Promise<void> {
    if (!this.webhookUrl || this.webhookUrl.trim() === "") {
      console.log("[DiscordNotifier] No webhook URL configured, skipping alert.");
      return;
    }

    const fields = report.quotes.map((q) => ({
      name: `${q.venueName}`,
      value: `Effective Price: **$${q.effectivePriceUsdcPerSol.toFixed(4)}**\nNet: **$${q.netRealizedPriceUsdc.toFixed(4)}** (Slip: ${q.slippageBps} bps)`,
      inline: true,
    }));

    const payload = {
      embeds: [
        {
          title: `⚡ Solana Price Divergence Detected: ${report.pair}`,
          color: report.arbOpportunity ? 0x00ff66 : 0x3b82f6,
          fields: [
            ...fields,
            {
              name: "Summary Breakdown",
              value: `🏆 **Top Venue**: ${report.bestVenue}\n📉 **Worst Venue**: ${report.worstVenue}\n📊 **Spread**: **${report.spreadBps.toFixed(2)} bps**`,
              inline: false,
            },
          ],
          footer: {
            text: `Slot: ${report.slot} | SolPriceEngine Enterprise`,
          },
          timestamp: report.timestamp,
        },
      ],
    };

    try {
      await axios.post(this.webhookUrl, payload, { timeout: 4000 });
      console.log(`[DiscordNotifier] Sent discrepancy alert for ${report.pair}`);
    } catch (err: any) {
      console.error("[DiscordNotifier] Failed to send webhook:", err.message);
    }
  }
}