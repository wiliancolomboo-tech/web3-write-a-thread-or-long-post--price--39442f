import axios from "axios";
import { Resend } from "resend";
import { ComparisonResult } from "./price-engine";

export interface DispatcherConfig {
  discordWebhookUrl?: string;
  resendApiKey?: string;
  alertEmailRecipient?: string;
}

export class EnterpriseDispatcher {
  private discordUrl?: string;
  private resend?: Resend;
  private recipient?: string;

  constructor(config: DispatcherConfig) {
    this.discordUrl = config.discordWebhookUrl;
    if (config.resendApiKey) {
      this.resend = new Resend(config.resendApiKey);
    }
    this.recipient = config.alertEmailRecipient;
  }

  /**
   * Broadcasts market price comparison metrics to Discord operations channel
   */
  public async dispatchToDiscord(result: ComparisonResult): Promise<void> {
    if (!this.discordUrl) return;

    const fields = result.quotes.map((q) => ({
      name: `🏛️ ${q.venueName}`,
      value: `**Out:** \`${(q.outAmount.toNumber() / 1e6).toFixed(4)} USDC\`\n` +
             `**Price:** \`$${q.effectivePrice.toFixed(4)}\`\n` +
             `**Impact:** \`${q.priceImpactPct.toFixed(3)}%\`\n` +
             `**Latency:** \`${q.executionLatencyMs}ms\``,
      inline: true,
    }));

    const payload = {
      embeds: [
        {
          title: "⚡ SolPrism Real-Time Price Comparison Telemetry",
          color: result.divergenceBps > 50 ? 0xff0033 : 0x00ff88,
          fields: [
            ...fields,
            {
              name: "🏆 Optimal Execution Venue",
              value: `**${result.optimalVenue.venueName}** (Out: ${(result.optimalVenue.outAmount.toNumber() / 1e6).toFixed(4)} USDC)`,
              inline: false,
            },
            {
              name: "📊 Venue Discrepancy",
              value: `\`${result.divergenceBps} BPS\` (${(result.divergenceBps / 100).toFixed(2)}%)`,
              inline: true,
            },
          ],
          footer: {
            text: `SolPrism Engine • Timestamp: ${new Date(result.timestamp).toISOString()}`,
          },
        },
      ],
    };

    try {
      await axios.post(this.discordUrl, payload, { timeout: 4000 });
    } catch (err: any) {
      console.error("[EnterpriseDispatcher] Failed to send Discord webhook:", err.message);
    }
  }

  /**
   * Sends critical discrepancy alerting via Resend
   */
  public async dispatchDivergenceAlertEmail(result: ComparisonResult): Promise<void> {
    if (!this.resend || !this.recipient) return;

    const emailHtml = `
      <div style="font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif; max-width: 640px; margin: 0 auto; background: #0d1117; color: #c9d1d9; border-radius: 8px; padding: 24px; border: 1px solid #30363d;">
        <h1 style="color: #58a6ff; font-size: 22px; border-bottom: 1px solid #21262d; padding-bottom: 12px;">SolPrism High Divergence Arbitrage Notice</h1>
        <p>A price divergence of <strong>${result.divergenceBps} basis points</strong> was detected across Solana venues.</p>
        
        <table style="width: 100%; border-collapse: collapse; margin-top: 16px;">
          <thead>
            <tr style="background-color: #161b22; color: #8b949e; text-align: left;">
              <th style="padding: 8px; border: 1px solid #30363d;">Venue</th>
              <th style="padding: 8px; border: 1px solid #30363d;">Calculated Return</th>
              <th style="padding: 8px; border: 1px solid #30363d;">Effective Price</th>
              <th style="padding: 8px; border: 1px solid #30363d;">Impact</th>
            </tr>
          </thead>
          <tbody>
            ${result.quotes.map(q => `
              <tr>
                <td style="padding: 8px; border: 1px solid #30363d; font-weight: bold; color: #f0f6fc;">${q.venueName}</td>
                <td style="padding: 8px; border: 1px solid #30363d;">${(q.outAmount.toNumber() / 1e6).toFixed(4)} USDC</td>
                <td style="padding: 8px; border: 1px solid #30363d;">$${q.effectivePrice.toFixed(4)}</td>
                <td style="padding: 8px; border: 1px solid #30363d;">${q.priceImpactPct.toFixed(3)}%</td>
              </tr>
            `).join('')}
          </tbody>
        </table>

        <div style="margin-top: 24px; padding: 12px; background: #1f242c; border-left: 4px solid #238636; border-radius: 4px;">
          <strong>Optimal Routing Decision:</strong> Route via <strong>${result.optimalVenue.venueName}</strong> for maximum realized yield.
        </div>
      </div>
    `;

    try {
      await this.resend.emails.send({
        from: "SolPrism Engine <alerts@solprism.io>",
        to: this.recipient,
        subject: `🚨 [SOLPRISM] High Discrepancy Detected: ${result.divergenceBps} BPS Divergence`,
        html: emailHtml,
      });
    } catch (err: any) {
      console.error("[EnterpriseDispatcher] Failed sending Resend email alert:", err.message);
    }
  }
}