pub mod dispatcher;
pub mod error;
pub mod hints;
pub mod matcher;
pub mod platform;
pub mod template;

pub use dispatcher::{Destination, WebhookDispatcher, WebhookDispatcherBuilder};
pub use error::WebhookError;
pub use hints::WithHints;
pub use matcher::MatcherRegistry;
pub use platform::{
    discord::Discord,
    telegram::{ParseMode, Telegram},
};
pub use template::TemplateEngine;
