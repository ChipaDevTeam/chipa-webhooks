use std::collections::HashMap;
use std::sync::Arc;

use serde::Deserialize;

use chipa_webhooks::{
    Destination,
    platform::{
        Platform, discord::Discord, generic::Generic, ntfy::Ntfy, slack::Slack, telegram::Telegram,
    },
};

use crate::error::{WebhookFfiError, into_config_error};

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum CredentialsConfig {
    Discord { webhook_url: String },
    Slack { webhook_url: String },
    Telegram { token: String, chat_id: i64 },
    Ntfy { url: String, topic: String },
    Generic { url: String },
}

impl CredentialsConfig {
    pub(crate) fn into_platform(self) -> Arc<dyn Platform> {
        match self {
            Self::Discord { webhook_url } => Arc::new(Discord::new(webhook_url)),
            Self::Slack { webhook_url } => Arc::new(Slack::new(webhook_url)),
            Self::Telegram { token, chat_id } => Arc::new(Telegram::new(token, chat_id)),
            Self::Ntfy { url, topic } => Arc::new(Ntfy::new(url, topic)),
            Self::Generic { url } => Arc::new(Generic::new(url)),
        }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct DestinationConfig {
    pub(crate) name: String,
    pub(crate) credentials: CredentialsConfig,
}

impl DestinationConfig {
    pub(crate) fn into_destination(self) -> Destination {
        let platform = self.credentials.into_platform();
        Destination {
            name: self.name,
            platform,
        }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct ClientConfig {
    pub(crate) destinations: Vec<DestinationConfig>,
    pub(crate) templates: Option<HashMap<String, String>>,
    pub(crate) capacity: Option<usize>,
}

impl ClientConfig {
    pub(crate) fn parse(json: &str) -> Result<Self, WebhookFfiError> {
        let config: Self = serde_json::from_str(json)
            .map_err(|e| into_config_error(e, "failed to parse config JSON"))?;

        if config.destinations.is_empty() {
            return Err(WebhookFfiError::Config {
                message: "config must contain at least one destination".to_owned(),
            });
        }

        let mut seen = std::collections::HashSet::new();
        for dest in &config.destinations {
            if !seen.insert(dest.name.as_str()) {
                return Err(WebhookFfiError::Config {
                    message: format!(
                        "duplicate destination name {:?} — each destination must have a unique name",
                        dest.name
                    ),
                });
            }
        }

        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_discord_config() {
        let json = r#"{
            "destinations": [
                { "name": "my-discord", "credentials": { "type": "discord", "webhook_url": "https://discord.com/api/webhooks/123/abc" } }
            ],
            "templates": { "default": "{{event}}: {{message}}" }
        }"#;
        let config = ClientConfig::parse(json).unwrap();
        assert_eq!(config.destinations.len(), 1);
        assert_eq!(config.destinations[0].name, "my-discord");
        assert!(config.templates.is_some());
    }

    #[test]
    fn valid_telegram_config() {
        let json = r#"{
            "destinations": [
                { "name": "tg", "credentials": { "type": "telegram", "token": "bot:token", "chat_id": -100123 } }
            ]
        }"#;
        let config = ClientConfig::parse(json).unwrap();
        assert_eq!(config.destinations[0].name, "tg");
        assert!(config.capacity.is_none());
    }

    #[test]
    fn missing_destinations_field() {
        let json = r#"{ "templates": { "default": "hi" } }"#;
        let err = ClientConfig::parse(json).unwrap_err();
        assert!(matches!(err, WebhookFfiError::Config { .. }));
    }

    #[test]
    fn empty_destinations_list() {
        let json = r#"{ "destinations": [] }"#;
        let err = ClientConfig::parse(json).unwrap_err();
        match err {
            WebhookFfiError::Config { message } => {
                assert!(
                    message.contains("at least one destination"),
                    "got: {message}"
                );
            }
            other => panic!("expected Config error, got {other:?}"),
        }
    }

    #[test]
    fn duplicate_destination_names() {
        let json = r#"{
            "destinations": [
                { "name": "dup", "credentials": { "type": "discord", "webhook_url": "https://a" } },
                { "name": "dup", "credentials": { "type": "discord", "webhook_url": "https://b" } }
            ]
        }"#;
        let err = ClientConfig::parse(json).unwrap_err();
        match err {
            WebhookFfiError::Config { message } => {
                assert!(message.contains("dup"), "got: {message}");
            }
            other => panic!("expected Config error, got {other:?}"),
        }
    }

    #[test]
    fn unknown_credentials_type() {
        let json = r#"{
            "destinations": [
                { "name": "x", "credentials": { "type": "carrier_pigeon", "url": "https://a" } }
            ]
        }"#;
        let err = ClientConfig::parse(json).unwrap_err();
        assert!(matches!(err, WebhookFfiError::Config { .. }));
    }
}
