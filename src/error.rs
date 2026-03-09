use thiserror::Error;

#[derive(Debug, Error)]
pub enum WebhookError {
    #[error("template not found: {0}")]
    TemplateNotFound(String),

    #[error("template render error: {0}")]
    TemplateRender(#[from] handlebars::RenderError),

    #[error("http error sending to {destination}: {source}")]
    Http {
        destination: String,
        source: reqwest::Error,
    },

    #[error("serialization error: {0}")]
    Serialize(#[from] serde_json::Error),

    #[error("template registration error: {0}")]
    TemplateRegister(#[from] handlebars::TemplateError),

    #[error("dispatcher channel closed")]
    ChannelClosed,
}
