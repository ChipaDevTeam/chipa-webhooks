use chipa_webhooks::{Destination, Discord, Generic, Telegram, WebhookDispatcher, WithHints};
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
        .destination(Destination::new(
            "telegram-test",
            Telegram::new("8671414424:AAEHU4k3ec2Z7u-v13W0yFE6Mngxzf2c2Wc", 8671414424),
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

#[tokio::test]
async fn test_runtime_template_mutations() {
    dotenvy::dotenv().ok();

    let webhook_url = std::env::var("DISCORD_WEBHOOK").expect("DISCORD_WEBHOOK not set in .env");

    // --- Phase 1: register 3 templates at build time and send one message each ---

    let dispatcher = WebhookDispatcher::builder()
        .template("alpha", "🅰️ **Alpha** — {{name}}: {{value}}")
        .template("beta", "🅱️ **Beta** — {{name}}: {{value}}")
        .template("gamma", "🅾️ **Gamma** — {{name}}: {{value}}")
        .destination(Destination::new(
            "discord",
            Discord::new(&webhook_url).with_username("Template Mutation Test"),
        ))
        .on_error(|e| eprintln!("webhook error: {e}"))
        .build()
        .expect("failed to build dispatcher");

    #[derive(Serialize)]
    struct Payload {
        name: String,
        value: String,
    }

    dispatcher
        .send_with_template_and_hints(
            "alpha",
            &Payload {
                name: "alpha-1".into(),
                value: "original".into(),
            },
            WithHints::new()
                .d_title("Phase 1 — Alpha")
                .d_color(0x3498db),
        )
        .await
        .expect("failed to send alpha");

    dispatcher
        .send_with_template_and_hints(
            "beta",
            &Payload {
                name: "beta-1".into(),
                value: "original".into(),
            },
            WithHints::new().d_title("Phase 1 — Beta").d_color(0x9b59b6),
        )
        .await
        .expect("failed to send beta");

    dispatcher
        .send_with_template_and_hints(
            "gamma",
            &Payload {
                name: "gamma-1".into(),
                value: "original".into(),
            },
            WithHints::new()
                .d_title("Phase 1 — Gamma")
                .d_color(0xe67e22),
        )
        .await
        .expect("failed to send gamma");

    // Flush ensures all Phase 1 HTTP requests complete before we mutate templates
    dispatcher.flush().await.expect("failed to flush");

    // --- Phase 2: remove gamma, update alpha + beta, add delta, send all three ---

    dispatcher.remove_template("gamma");

    dispatcher
        .update_template("alpha", "🅰️ **Alpha (updated)** — {{name}}: {{value}} ✏️")
        .expect("failed to update alpha");

    dispatcher
        .update_template("beta", "🅱️ **Beta (updated)** — {{name}}: {{value}} ✏️")
        .expect("failed to update beta");

    dispatcher
        .register_template("delta", "🔷 **Delta (new)** — {{name}}: {{value}} 🆕")
        .expect("failed to register delta");

    dispatcher
        .send_with_template_and_hints(
            "alpha",
            &Payload {
                name: "alpha-2".into(),
                value: "updated".into(),
            },
            WithHints::new()
                .d_title("Phase 2 — Alpha updated")
                .d_color(0x2ecc71),
        )
        .await
        .expect("failed to send updated alpha");

    dispatcher
        .send_with_template_and_hints(
            "beta",
            &Payload {
                name: "beta-2".into(),
                value: "updated".into(),
            },
            WithHints::new()
                .d_title("Phase 2 — Beta updated")
                .d_color(0x2ecc71),
        )
        .await
        .expect("failed to send updated beta");

    dispatcher
        .send_with_template_and_hints(
            "delta",
            &Payload {
                name: "delta-1".into(),
                value: "brand new".into(),
            },
            WithHints::new()
                .d_title("Phase 2 — Delta new")
                .d_color(0x1abc9c),
        )
        .await
        .expect("failed to send delta");

    // Verify gamma is gone — should return an error, not panic
    let result = dispatcher
        .send_with_template(
            "gamma",
            &Payload {
                name: "ghost".into(),
                value: "should fail".into(),
            },
        )
        .await;
    assert!(result.is_err(), "gamma should have been removed");

    dispatcher.shutdown().await;
}

#[tokio::test]
async fn test_multi_platform_fanout() {
    dotenvy::dotenv().ok();

    let discord_url = std::env::var("DISCORD_WEBHOOK").expect("DISCORD_WEBHOOK not set in .env");
    let webhook_site_url =
        std::env::var("WEBHOOK_SITE_URL").expect("WEBHOOK_SITE_URL not set in .env");

    #[derive(Serialize)]
    struct Signal {
        asset: String,
        action: String,
        entry_price: f64,
        timeframe: String,
        note: String,
    }

    let mut dispatcher = WebhookDispatcher::builder()
        .template(
            "signal",
            "**{{action}}** — {{asset}}\nEntry: `{{entry_price}}`\nTimeframe: {{timeframe}}\nNote: {{note}}",
        )
        .destination(Destination::new(
            "discord",
            Discord::new(discord_url).with_username("Chipa Fan-out Test"),
        ))
        .destination(Destination::new(
            "webhook-site",
            Generic::new(webhook_site_url).with_body_key("content"),
        ))
        .on_error(|e| eprintln!("fanout error: {e}"))
        .build()
        .expect("failed to build dispatcher");

    dispatcher.register_rule(|s: &Signal| match s.action.as_str() {
        "Buy" | "StrongBuy" => "signal",
        _ => "signal",
    });

    // Message 1 — Buy signal, sent to Discord (with embed color) and webhook.site simultaneously
    dispatcher
        .send_with_hints(
            &Signal {
                asset: "BTCUSDT".into(),
                action: "Buy".into(),
                entry_price: 67_420.50,
                timeframe: "15m".into(),
                note: "fan-out test — should appear on Discord AND webhook.site".into(),
            },
            WithHints::new()
                .d_color(0x2ecc71)
                .d_title("📈 BUY — BTCUSDT"),
        )
        .await
        .expect("failed to send buy signal");

    // Message 2 — Sell signal
    dispatcher
        .send_with_hints(
            &Signal {
                asset: "#EURUSD_otc".into(),
                action: "Sell".into(),
                entry_price: 1.08210,
                timeframe: "5m".into(),
                note: "second fan-out message".into(),
            },
            WithHints::new()
                .d_color(0xe74c3c)
                .d_title("📉 SELL — #EURUSD_otc"),
        )
        .await
        .expect("failed to send sell signal");

    dispatcher.shutdown().await;
}
