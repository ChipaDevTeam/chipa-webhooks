use chipa_webhooks::{Destination, Discord, WebhookDispatcher, WithHints};
use serde::Serialize;

const COLOR_BUY: u32 = 0x2ecc71;
const COLOR_SELL: u32 = 0xe74c3c;
const COLOR_NEUTRAL: u32 = 0x95a5a6;

#[derive(Serialize)]
struct TradeSignal {
    asset: String,
    action: String,
    entry_price: f64,
    timeframe: String,
    instance_id: String,
    timestamp: String,
}

#[derive(Serialize)]
struct RandomEvent {
    title: String,
    message: String,
    value: u32,
}

#[tokio::test]
async fn test_discord_trading_signal_and_random() {
    dotenvy::dotenv().ok();

    let webhook_url = std::env::var("DISCORD_WEBHOOK").expect("DISCORD_WEBHOOK not set in .env");

    let trading_template = "\
**Asset:** {{asset}}\n\
**Action:** {{action}}\n\
**Entry Price:** `{{entry_price}}`\n\
**Timeframe:** {{timeframe}}\n\
**Instance:** {{instance_id}}\n\
**Time:** {{timestamp}}";

    let random_template = "\
**Title:** {{title}}\n\
**Message:** {{message}}\n\
**Value:** {{value}}";

    let mut dispatcher = WebhookDispatcher::builder()
        .template("default", random_template)
        .template("trade_buy", trading_template)
        .template("trade_sell", trading_template)
        .destination(Destination::new(
            "discord-test",
            Discord::new(webhook_url).with_username("Chipa Webhooks Test"),
        ))
        .on_error(|e| eprintln!("webhook error: {e}"))
        .build()
        .expect("failed to build dispatcher");

    dispatcher.register_rule(|e: &TradeSignal| match e.action.as_str() {
        "Buy" | "StrongBuy" => "trade_buy",
        "Sell" | "StrongSell" => "trade_sell",
        _ => "default",
    });

    // --- Message 1: Buy signal — matcher resolves to "trade_buy", green embed ---
    let buy_signal = TradeSignal {
        asset: "#EURUSD_otc".to_owned(),
        action: "Buy".to_owned(),
        entry_price: 1.08321,
        timeframe: "5m".to_owned(),
        instance_id: "inst_018c_abc123".to_owned(),
        timestamp: "2025-10-14 12:00:00 UTC".to_owned(),
    };

    dispatcher
        .send_with_hints(
            &buy_signal,
            WithHints::new()
                .d_color(COLOR_BUY)
                .d_title("📈 BUY — #EURUSD_otc"),
        )
        .await
        .expect("failed to send buy signal");

    // --- Message 2: Sell signal — matcher resolves to "trade_sell", red embed ---
    let sell_signal = TradeSignal {
        asset: "BTCUSDT".to_owned(),
        action: "Sell".to_owned(),
        entry_price: 67_420.50,
        timeframe: "15m".to_owned(),
        instance_id: "inst_018c_def456".to_owned(),
        timestamp: "2025-10-14 12:15:00 UTC".to_owned(),
    };

    dispatcher
        .send_with_hints(
            &sell_signal,
            WithHints::new()
                .d_color(COLOR_SELL)
                .d_title("📉 SELL — BTCUSDT"),
        )
        .await
        .expect("failed to send sell signal");

    // --- Message 3: Random event — explicit template, neutral grey embed ---
    let random_event = RandomEvent {
        title: "System Check".to_owned(),
        message: "chipa-webhooks integration test passed 🚀".to_owned(),
        value: 42,
    };

    dispatcher
        .send_with_template_and_hints(
            "default",
            &random_event,
            WithHints::new()
                .d_color(COLOR_NEUTRAL)
                .d_title("🎲 Random Event"),
        )
        .await
        .expect("failed to send random event");

    // Graceful shutdown: drains the queue and waits for all HTTP requests to finish
    dispatcher.shutdown().await;
}
