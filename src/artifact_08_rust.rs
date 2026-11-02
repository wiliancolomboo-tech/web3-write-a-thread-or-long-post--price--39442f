use anyhow::{Context, Result};
use log::{error, info, warn};
use reqwest::Client;
use serde_json::json;
use sol_price_guard_core::comparator::PriceComparator;
use sol_price_guard_core::math::PriceMath;
use sol_price_guard_core::types::{PriceQuote, VenueType};
use std::env;
use std::time::Duration;
use tokio::time::sleep;

/// Enterprise Webhook Dispatcher
pub struct WebhookNotifier {
    client: Client,
    discord_webhook_url: Option<String>,
    slack_webhook_url: Option<String>,
    resend_api_key: Option<String>,
    alert_recipient_email: Option<String>,
}

impl WebhookNotifier {
    pub fn from_env() -> Self {
        Self {
            client: Client::builder().timeout(Duration::from_secs(5)).build().unwrap(),
            discord_webhook_url: env::var("DISCORD_WEBHOOK_URL").ok(),
            slack_webhook_url: env::var("SLACK_WEBHOOK_URL").ok(),
            resend_api_key: env::var("RESEND_API_KEY").ok(),
            alert_recipient_email: env::var("ALERT_RECIPIENT_EMAIL").ok(),
        }
    }

    pub async fn dispatch_alert(
        &self,
        pair: &str,
        spread_bps: f64,
        min_v: VenueType,
        min_p: f64,
        max_v: VenueType,
        max_p: f64,
        slot: u64,
    ) {
        let msg = format!(
            "🚨 **[SolPriceGuard Alert]** Divergence Detected on **{}**!\n\
             • Slot: `{}`\n\
             • Spread: `{:.2} bps`\n\
             • Low Venue: `{:?}` @ **${:.4}**\n\
             • High Venue: `{:?}` @ **${:.4}**\n\
             • Arbitrage Viable: **YES** (Over 25 bps threshold)",
            pair, slot, spread_bps, min_v, min_p, max_v, max_p
        );

        // 1. Dispatch to Discord
        if let Some(ref url) = self.discord_webhook_url {
            let payload = json!({ "content": msg, "username": "SolPriceGuard Bot" });
            if let Err(e) = self.client.post(url).json(&payload).send().await {
                error!("Failed to notify Discord: {:?}", e);
            } else {
                info!("Discord webhook alert dispatched.");
            }
        }

        // 2. Dispatch to Slack
        if let Some(ref url) = self.slack_webhook_url {
            let payload = json!({ "text": msg });
            if let Err(e) = self.client.post(url).json(&payload).send().await {
                error!("Failed to notify Slack: {:?}", e);
            } else {
                info!("Slack webhook alert dispatched.");
            }
        }

        // 3. Dispatch to Resend Enterprise Email API
        if let (Some(ref api_key), Some(ref recipient)) =
            (&self.resend_api_key, &self.alert_recipient_email)
        {
            let resend_url = "https://api.resend.com/emails";
            let email_payload = json!({
                "from": "alerts@solpriceguard.io",
                "to": [recipient],
                "subject": format!("CRITICAL: Price Divergence {} bps on {}", spread_bps, pair),
                "html": format!(
                    "<h3>Solana Price Divergence Event</h3><p>{}</p><hr><p>Slot: {}</p>",
                    msg.replace("\n", "<br/>"),
                    slot
                )
            });

            let res = self
                .client
                .post(resend_url)
                .bearer_auth(api_key)
                .json(&email_payload)
                .send()
                .await;

            match res {
                Ok(r) if r.status().is_success() => info!("Resend alert email sent successfully."),
                Ok(r) => warn!("Resend API returned non-success: {:?}", r.status()),
                Err(e) => error!("Failed to send email via Resend: {:?}", e),
            }
        }
    }
}

/// Simulated Multi-Venue Feeder with real market mechanics
pub struct MarketFeedSimulator;

impl MarketFeedSimulator {
    pub async fn fetch_live_quotes(slot: u64) -> Vec<PriceQuote> {
        let now = chrono::Utc::now().timestamp_millis();
        
        // Base SOL/USDC price around $178.50 with venue-specific micro-variances
        vec![
            PriceQuote {
                venue: VenueType::PhoenixCLOB,
                token_pair: "SOL/USDC".into(),
                price: 178.45,
                confidence_or_spread: 0.02,
                liquidity_depth_usd: 850_000.0,
                slot,
                timestamp_ms: now,
            },
            PriceQuote {
                venue: VenueType::OrcaWhirlpool,
                token_pair: "SOL/USDC".into(),
                price: 178.95, // CLMM lag or tick skew
                confidence_or_spread: 0.01,
                liquidity_depth_usd: 2_400_000.0,
                slot,
                timestamp_ms: now,
            },
            PriceQuote {
                venue: VenueType::PythOracle,
                token_pair: "SOL/USDC".into(),
                price: 178.48,
                confidence_or_spread: 0.015,
                liquidity_depth_usd: 10_000_000.0,
                slot,
                timestamp_ms: now,
            },
            PriceQuote {
                venue: VenueType::JupiterRouting,
                token_pair: "SOL/USDC".into(),
                price: 178.52,
                confidence_or_spread: 0.04,
                liquidity_depth_usd: 5_000_000.0,
                slot,
                timestamp_ms: now,
            },
        ]
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    info!("Starting SolPriceGuard - Solana Price Comparison & Routing Daemon");

    let notifier = WebhookNotifier::from_env();
    let mut current_slot: u64 = 280_450_100;

    for loop_count in 1..=3 {
        current_slot += 1;
        info!("--- Evaluating Slot: {} (Cycle {}) ---", current_slot, loop_count);

        let quotes = MarketFeedSimulator::fetch_live_quotes(current_slot).await;
        let matrix = PriceComparator::evaluate(quotes)
            .context("Failed to compute price comparison matrix")?;

        info!("Pair: {}", matrix.token_pair);
        info!("Price Span: ${:.4} (via {:?}) -> ${:.4} (via {:?})", 
              matrix.min_price, matrix.min_price_venue, matrix.max_price, matrix.max_price_venue);
        info!("Spread: {:.2} bps | Arbitrage Viable: {}", matrix.spread_bps, matrix.is_arbitrage_viable);
        info!("Optimal Route to Buy: {:?}", matrix.best_execution_venue);

        // Threshold check: alert if spread > 20 bps
        if matrix.spread_bps > 20.0 {
            warn!("Cross-venue spread exceeded 20 bps! Triggering notification bus...");
            notifier
                .dispatch_alert(
                    &matrix.token_pair,
                    matrix.spread_bps,
                    matrix.min_price_venue,
                    matrix.min_price,
                    matrix.max_price_venue,
                    matrix.max_price,
                    matrix.slot,
                )
                .await;
        }

        sleep(Duration::from_millis(1500)).await;
    }

    info!("Daemon operational check completed successfully.");
    Ok(())
}