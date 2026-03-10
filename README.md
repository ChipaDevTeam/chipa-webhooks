# chipa-webhooks

A fast, non-blocking webhook dispatch crate with a handlebars-powered template engine. Built for high-frequency trading platforms where the dispatch path must never block the main thread.

## Features

- **Non-blocking dispatch** — `send` pushes to a `kanal` channel and returns immediately. All HTTP work happens in a background task.
- **Handlebars templates** — register named templates, render any `serde::Serialize` type into them.
- **TypeId-based matcher** — register a rule closure per type, `send` resolves the right template automatically.
- **Runtime template mutations** — add, update, or remove templates while the dispatcher is running.
- **Fan-out to multiple destinations** — one send reaches Discord, Telegram, Slack, Ntfy, and any other platform concurrently.
- **Platform hints** — pass per-message metadata like embed color or title via `WithHints` without polluting your data types.
- **Graceful shutdown** — `shutdown().await` drains the queue before returning.
- **Flush barrier** — `flush().await` waits for all queued jobs to complete without closing the channel.
- **Isolated errors** — a failing destination never affects others. Errors are reported via an `on_error` callback.

## Supported Platforms

| Platform | Type | Notes |
|---|---|---|
| Discord | ✅ | Webhook URL, embed color, embed title, username |
| Telegram | ✅ | Bot token + chat ID, MarkdownV2 / HTML / Plain, silent, disable preview |
| Slack | ✅ | Incoming webhook URL, username, icon emoji |
| Ntfy | ✅ | Self-hosted or ntfy.sh, title, priority, tags |
| Generic HTTP | ✅ | Plain JSON POST to any URL, configurable body key |

### Platforms planned

| Platform | Notes |
|---|---|
| Microsoft Teams | Adaptive Cards, high demand in finance orgs |
| Mattermost | Slack-compatible payload |
| Pushover | Mobile push, popular with individual traders |
| PagerDuty | On-call alerting, severity levels map well to trade events |
| OpsGenie | Popular PagerDuty alternative |
| Lark / Feishu | Large user base in Asian markets |
| DingTalk | Same target market as Lark |
| WeChat Work | Enterprise WeChat, relevant for CN trading firms |
| Email (SMTP) | Via `lettre`, useful for low-frequency critical alerts |

## Installation

```toml
[dependencies]
chipa-webhooks = "0.1"
```

## Quick Start

```rust
use chipa_webhooks::{Destination, Discord, WebhookDispatcher, WithHints};
use serde::Serialize;

#[derive(Serialize)]
struct TradeSignal {
    asset: String,
    action: String,
    entry_price: f64,
}

#[tokio::main]
async fn main() {
    let mut dispatcher = WebhookDispatcher::builder()
        .template("signal_buy",  "📈 **BUY** — {{asset}} @ {{entry_price}}")
        .template("signal_sell", "📉 **SELL** — {{asset}} @ {{entry_price}}")
        .template("default",     "⚪ {{action}} — {{asset}}")
        .destination(Destination::new(
            "discord",
            Discord::new("https://discord.com/api/webhooks/...").with_username("Chipa"),
        ))
        .on_error(|e| eprintln!("webhook error: {e}"))
        .build()
        .expect("failed to build dispatcher");

    dispatcher.register_rule(|e: &TradeSignal| match e.action.as_str() {
        "Buy" | "StrongBuy"   => "signal_buy",
        "Sell" | "StrongSell" => "signal_sell",
        _                     => "default",
    });

    let signal = TradeSignal {
        asset: "BTCUSDT".into(),
        action: "Buy".into(),
        entry_price: 67_420.50,
    };

    // Matcher resolves "signal_buy" automatically
    dispatcher.send(&signal).await.unwrap();

    // Explicit template + platform hints
    dispatcher
        .send_with_hints(
            &signal,
            WithHints::new()
                .d_color(0x2ecc71)
                .d_title("📈 BUY — BTCUSDT"),
        )
        .await
        .unwrap();

    dispatcher.shutdown().await;
}
```

## Send Methods

| Method | Template source | Hints |
|---|---|---|
| `send(&event)` | Matcher (TypeId lookup) | — |
| `send_with_hints(&event, hints)` | Matcher (TypeId lookup) | ✅ |
| `send_with_template("name", &event)` | Explicit | — |
| `send_with_template_and_hints("name", &event, hints)` | Explicit | ✅ |

All four methods are `async` and return immediately after queuing — the HTTP request happens in the background.

## Platform Hints

`WithHints` is a builder that attaches per-message metadata. Hints are stripped from the template data before rendering so they never appear in the message text.

```rust
WithHints::new()
    // Discord
    .d_color(0xe74c3c)           // embed color (u32 RGB)
    .d_title("📉 SELL signal")   // embed title

    // Telegram
    .tg_silent()                 // disable notification sound
    .tg_disable_preview()        // disable link preview

    // Slack
    .slack_username("ChipaBot")  // override bot name for this message
    .slack_emoji(":chart:")      // override icon emoji for this message

    // Ntfy
    .ntfy_title("Trade Alert")   // notification title
    .ntfy_priority(4)            // 1 (min) to 5 (max)
    .ntfy_tags("trading,btc")    // comma-separated, additive with struct-level tags
```

### Discord color reference

| Meaning | Hex |
|---|---|
| Buy / Win / OK | `0x2ecc71` |
| Sell / Loss / Error | `0xe74c3c` |
| Hold / Neutral | `0x95a5a6` |
| Warning | `0xe67e22` |
| Info | `0x3498db` |

## Multi-Platform Fan-out

One dispatcher fans out to all destinations concurrently. Each destination is fully isolated — a timeout or error on one never delays or affects the others.

```rust
use chipa_webhooks::{Destination, Discord, Generic, Ntfy, Slack, Telegram, WebhookDispatcher, WithHints};
use serde::Serialize;

#[derive(Serialize)]
struct Signal {
    asset: String,
    action: String,
    entry_price: f64,
}

#[tokio::main]
async fn main() {
    let dispatcher = WebhookDispatcher::builder()
        .template("signal", "**{{action}}** — {{asset}} @ {{entry_price}}")
        .destination(Destination::new(
            "discord",
            Discord::new("https://discord.com/api/webhooks/...").with_username("Chipa"),
        ))
        .destination(Destination::new(
            "telegram",
            Telegram::new("bot_token", -1001234567890),
        ))
        .destination(Destination::new(
            "slack",
            Slack::new("https://hooks.slack.com/services/...").with_icon_emoji(":chart_with_upwards_trend:"),
        ))
        .destination(Destination::new(
            "ntfy",
            Ntfy::new("https://ntfy.sh", "trading-alerts")
                .with_priority(4)
                .with_tags(["trading", "signal"]),
        ))
        .destination(Destination::new(
            "webhook-site",
            Generic::new("https://webhook.site/your-uuid").with_body_key("content"),
        ))
        .on_error(|e| eprintln!("webhook error [{e}]"))
        .build()
        .expect("failed to build dispatcher");

    dispatcher
        .send_with_template_and_hints(
            "signal",
            &Signal {
                asset: "BTCUSDT".into(),
                action: "Buy".into(),
                entry_price: 67_420.50,
            },
            WithHints::new()
                .d_color(0x2ecc71)
                .d_title("📈 BUY — BTCUSDT")
                .ntfy_priority(5)
                .slack_emoji(":rocket:"),
        )
        .await
        .unwrap();

    dispatcher.shutdown().await;
}
```

## Runtime Template Mutations

Templates can be changed while the dispatcher is running. Always call `flush().await` before mutating to ensure all queued sends complete first — sends are fire-and-forget into a channel, so without a flush, in-flight jobs may render against the mutated template.

```rust
// Phase 1 — send with original templates
dispatcher.send_with_template("report", &event).await.unwrap();

// Wait for all queued HTTP requests to finish before mutating
dispatcher.flush().await.unwrap();

// Phase 2 — mutate, then send
dispatcher.update_template("report", "📊 NEW FORMAT — {{asset}}: {{value}}").unwrap();
dispatcher.register_template("alert",  "🚨 ALERT — {{message}}").unwrap();
dispatcher.remove_template("old");

dispatcher.send_with_template("report", &event).await.unwrap();
dispatcher.send_with_template("alert",  &other).await.unwrap();
```

## Graceful Shutdown

```rust
// Closes the channel, drains every queued job to completion, then returns.
// No messages are lost.
dispatcher.shutdown().await;
```

## Custom Platform

Implement the `Platform` trait to add any HTTP-based platform:

```rust
use chipa_webhooks::platform::Platform;
use serde_json::{Map, Value, json};

struct MyPlatform {
    url: String,
}

impl Platform for MyPlatform {
    fn build_payload(&self, rendered: &str, hints: &Map<String, Value>) -> Value {
        json!({
            "text":     rendered,
            "priority": hints.get("__ntfy_priority").cloned().unwrap_or(Value::Null),
        })
    }

    fn endpoint(&self) -> &str {
        &self.url
    }
}

// Use it like any built-in platform
Destination::new("my-platform", MyPlatform { url: "https://...".into() });
```

## Telegram Setup

1. Create a bot via [@BotFather](https://t.me/BotFather) and copy the token.
2. Start a conversation with the bot (send `/start`).
3. Retrieve your chat ID:
   ```
   https://api.telegram.org/bot<TOKEN>/getUpdates
   ```
   Look for `"chat": { "id": 123456789 }` in the response.

```rust
use chipa_webhooks::{Destination, Telegram, ParseMode};

Destination::new(
    "telegram",
    Telegram::new("7123456789:AAFxxx...", -1001234567890)
        .with_parse_mode(ParseMode::Html),
)
```

## Ntfy Setup

Works with the public [ntfy.sh](https://ntfy.sh) server or any self-hosted instance.

```rust
use chipa_webhooks::{Destination, Ntfy};

// Public ntfy.sh
Destination::new(
    "ntfy",
    Ntfy::new("https://ntfy.sh", "my-trading-alerts")
        .with_title("Trade Alert")
        .with_priority(4)
        .with_tags(["trading", "signal", "btc"]),
)

// Self-hosted
Destination::new(
    "ntfy-self",
    Ntfy::new("https://ntfy.myserver.com", "alerts"),
)
```

## Architecture

```
Caller
  │
  │  send(&event)  ← async, returns after channel push
  ▼
kanal bounded channel (capacity: 1024 by default)
  │
  ▼
Background task (tokio::spawn)
  ├── TemplateEngine::render()        ← handlebars, Arc<RwLock> shared with caller
  └── fan-out via join_all
        ├── Platform::build_payload() + reqwest POST  →  Discord
        ├── Platform::build_payload() + reqwest POST  →  Telegram
        ├── Platform::build_payload() + reqwest POST  →  Slack
        ├── Platform::build_payload() + reqwest POST  →  Ntfy
        └── Platform::build_payload() + reqwest POST  →  ...
```

- The `TemplateEngine` is shared between the caller and the background task via `Arc<RwLock>`. The caller holds a write lock only during `register`/`update`/`remove` calls. The background task holds a read lock only during `render`.
- A `Flush` sentinel job is enqueued by `flush()`. When the background task reaches it, all prior jobs are guaranteed to have completed their HTTP fan-outs.

## Environment Variables (for tests)

```env
DISCORD_WEBHOOK=https://discord.com/api/webhooks/<id>/<token>
WEBHOOK_SITE_URL=https://webhook.site/<uuid>
```

## Dependencies

| Crate | Role |
|---|---|
| `kanal` | High-performance async channel |
| `tokio` | Async runtime |
| `reqwest` | HTTP client (rustls, no OpenSSL) |
| `handlebars` | Template engine |
| `serde` + `serde_json` | Serialization |
| `futures` | `join_all` for concurrent fan-out |
| `thiserror` | Error type derivation |
| `tracing` | Structured logging |