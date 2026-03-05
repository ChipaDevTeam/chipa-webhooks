use serde_json::{Map, Value, json};

use super::Platform;

pub struct Discord {
    webhook_url: String,
    username: Option<String>,
}

impl Discord {
    pub fn new(webhook_url: impl Into<String>) -> Self {
        Self {
            webhook_url: webhook_url.into(),
            username: None,
        }
    }

    pub fn with_username(mut self, username: impl Into<String>) -> Self {
        self.username = Some(username.into());
        self
    }
}

impl Platform for Discord {
    fn build_payload(&self, rendered: &str, hints: &Map<String, Value>) -> Value {
        let mut embed = json!({ "description": rendered });

        if let Some(color) = hints.get("__d_color") {
            embed["color"] = color.clone();
        }

        if let Some(title) = hints.get("__d_title") {
            embed["title"] = title.clone();
        }

        let mut payload = json!({ "embeds": [embed] });

        if let Some(username) = &self.username {
            payload["username"] = Value::String(username.clone());
        }

        payload
    }

    fn endpoint(&self) -> &str {
        &self.webhook_url
    }
}
