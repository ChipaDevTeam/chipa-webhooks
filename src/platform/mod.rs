use serde_json::{Map, Value};

use crate::error::WebhookError;

pub mod discord;
pub mod generic;
pub mod ntfy;
pub mod slack;
pub mod telegram;

pub trait Platform: Send + Sync {
    fn build_payload(&self, rendered: &str, hints: &Map<String, Value>) -> Value;
    fn endpoint(&self) -> &str;
}

pub async fn post(
    client: &reqwest::Client,
    platform: &dyn Platform,
    rendered: &str,
    hints: &Map<String, Value>,
    destination_name: &str,
) -> Result<(), WebhookError> {
    let payload = platform.build_payload(rendered, hints);

    let response = client
        .post(platform.endpoint())
        .json(&payload)
        .send()
        .await
        .map_err(|e| WebhookError::Http {
            destination: destination_name.to_owned(),
            source: e,
        })?;

    if !response.status().is_success() {
        let status = response.status();
        tracing::warn!(destination = destination_name, status = %status, "webhook returned non-2xx");
    }

    Ok(())
}
