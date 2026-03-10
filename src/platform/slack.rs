use serde_json::{Map, Value, json};

use super::Platform;

pub struct Slack {
    webhook_url: String,
    username: Option<String>,
    icon_emoji: Option<String>,
}

impl Slack {
    pub fn new(webhook_url: impl Into<String>) -> Self {
        Self {
            webhook_url: webhook_url.into(),
            username: None,
            icon_emoji: None,
        }
    }

    pub fn with_username(mut self, username: impl Into<String>) -> Self {
        self.username = Some(username.into());
        self
    }

    pub fn with_icon_emoji(mut self, emoji: impl Into<String>) -> Self {
        self.icon_emoji = Some(emoji.into());
        self
    }
}

impl Platform for Slack {
    fn build_payload(&self, rendered: &str, hints: &Map<String, Value>) -> Value {
        let mut payload = json!({ "text": rendered });

        let username = hints
            .get("__slack_username")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .or_else(|| self.username.clone());

        if let Some(u) = username {
            payload["username"] = Value::String(u);
        }

        let icon = hints
            .get("__slack_emoji")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .or_else(|| self.icon_emoji.clone());

        if let Some(i) = icon {
            payload["icon_emoji"] = Value::String(i);
        }

        payload
    }

    fn endpoint(&self) -> &str {
        &self.webhook_url
    }
}
