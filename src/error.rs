use std::fmt;

#[derive(Debug)]
pub enum WebhookError {
    TemplateNotFound(String),
    TemplateRender(handlebars::RenderError),
    Http {
        destination: String,
        source: reqwest::Error,
    },
    Serialize(serde_json::Error),
    ChannelClosed,
}

impl fmt::Display for WebhookError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TemplateNotFound(name) => write!(f, "template not found: {name}"),
            Self::TemplateRender(e) => write!(f, "template render error: {e}"),
            Self::Http {
                destination,
                source,
            } => {
                write!(f, "http error sending to {destination}: {source}")
            }
            Self::Serialize(e) => write!(f, "serialization error: {e}"),
            Self::ChannelClosed => write!(f, "dispatcher channel closed"),
        }
    }
}

impl std::error::Error for WebhookError {}

impl From<handlebars::RenderError> for WebhookError {
    fn from(e: handlebars::RenderError) -> Self {
        Self::TemplateRender(e)
    }
}

impl From<serde_json::Error> for WebhookError {
    fn from(e: serde_json::Error) -> Self {
        Self::Serialize(e)
    }
}
