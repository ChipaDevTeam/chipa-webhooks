use chipa_webhooks_ffi::{MatcherCallback, WebhookClient};
use std::sync::Arc;

/// 1. Define a custom Matcher (Rule)
/// This intercepts `dispatch` calls and decides which template to use based on the payload.
/// If it returns `None`, it falls back to the next matcher or "default".
struct AlertMatcher;

impl MatcherCallback for AlertMatcher {
    fn match_template(&self, payload_json: String) -> Option<String> {
        // The send/dispatch methods take a raw JSON string from the FFI boundary.
        // We can parse it here to inspect its contents.
        if let Ok(data) = serde_json::from_str::<serde_json::Value>(&payload_json) {
            if data.get("level").and_then(|v| v.as_str()) == Some("critical") {
                return Some("alerts".to_owned());
            }
        }
        None
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Chipa Webhooks Rust FFI Example ---\n");

    // 2. Configuration with Multiple Destinations and Templates
    // The client takes a JSON string config.
    let config_json = r#"{
        "destinations": [
            {
                "name": "my-discord",
                "credentials": {
                    "type": "discord",
                    "webhook_url": "https://discord.com/api/webhooks/0000/fake-token"
                }
            },
            {
                "name": "my-telegram",
                "credentials": {
                    "type": "telegram",
                    "token": "123456:ABC-DEF1234ghIkl-zyx57W2v1u123ew11",
                    "chat_id": -100123456789
                }
            }
        ],
        "templates": {
            "default": "New event: {{message}}",
            "alerts": "🚨 [ALERT] 🚨 {{message}}"
        }
    }"#;

    // Initialize the client
    let client = WebhookClient::new(config_json.to_owned())?;

    // 3. Register the custom rule
    client.register_matcher(Box::new(AlertMatcher));

    // 4. Sending Sync Messages
    // The `send` and `dispatch` methods take a raw JSON string.
    // Internally, the Rust core parses this string into a `serde_json::Value`
    // to render the Handlebars template with the provided data.
    println!("[Sync] Sending direct message using 'default' template...");
    let payload_sync = serde_json::json!({
        "message": "Hello from Sync Send!"
    })
    .to_string();
    client.send_sync("default".to_owned(), payload_sync)?;

    println!("[Sync] Dispatching a critical alert...");
    // This will be routed to the 'alerts' template by our AlertMatcher.
    let payload_alert = serde_json::json!({
        "level": "critical",
        "message": "The database is down!"
    })
    .to_string();
    client.dispatch_sync(payload_alert)?;

    println!("[Sync] Dispatching a normal info event...");
    // This won't match our AlertMatcher, so it falls back to the 'default' template.
    let payload_info = serde_json::json!({
        "level": "info",
        "message": "Routine backup completed."
    })
    .to_string();
    client.dispatch_sync(payload_info)?;

    // 5. Sending Async Messages
    // When the `async` feature is enabled, you can use `.send()` and `.dispatch()` directly.
    println!("\n[Async] Sending direct message using 'alerts' template...");
    let payload_async_warn = serde_json::json!({
        "message": "Async system warning!"
    })
    .to_string();
    client.send("alerts".to_owned(), payload_async_warn).await?;

    println!("[Async] Dispatching a critical alert...");
    let payload_async_crit = serde_json::json!({
        "level": "critical",
        "message": "CPU usage at 99%!"
    })
    .to_string();
    client.dispatch(payload_async_crit).await?;

    // 6. Graceful Shutdown
    // This flushes any pending HTTP requests in the background dispatcher queue.
    println!("\nShutting down gracefully...");
    client.shutdown().await;
    println!("Done!");

    Ok(())
}
