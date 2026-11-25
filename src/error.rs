use thiserror::Error;

#[derive(Debug, Error)]
pub enum WebhookError {
    #[error("template not found: {0}")]
    TemplateNotFound(String),

    #[error("template render error: {0}")]
    TemplateRender(#[from] handlebars::RenderError),

    /// Transport-level failure. The request URL is stripped from `source`, so
    /// webhook tokens never leak through `Display`/`Debug`.
    #[error("http error sending to {destination}: {source}")]
    Http {
        destination: String,
        source: reqwest::Error,
    },

    /// The destination answered with a non-2xx status (after retries).
    /// Contains no URL; `body_excerpt` is truncated.
    #[error("{destination} responded with status {status}: {body_excerpt}")]
    Status {
        destination: String,
        status: u16,
        body_excerpt: String,
    },

    #[error("serialization error: {0}")]
    Serialize(#[from] serde_json::Error),

    #[error("template registration error: {0}")]
    TemplateRegister(#[from] handlebars::TemplateError),

    #[error("dispatcher channel closed")]
    ChannelClosed,

    /// Non-blocking enqueue failed because the queue is full; the message was dropped.
    #[error("dispatcher queue full, message dropped")]
    QueueFull,

    /// A destination's own queue was full; the payload was dropped for that destination.
    #[error("destination {0} queue full, message dropped")]
    DestinationQueueFull(String),

    /// A blocking call (`flush_blocking` / `shutdown_blocking`) hit its deadline.
    #[error("timed out waiting for the dispatcher")]
    Timeout,

    #[error("failed to start dispatcher runtime: {0}")]
    Runtime(String),

    #[error("failed to build http client: {0}")]
    ClientBuild(reqwest::Error),
}
