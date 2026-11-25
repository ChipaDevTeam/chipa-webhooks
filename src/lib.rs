mod delivery;
pub mod dispatcher;
pub mod error;
pub mod hints;
pub mod limiter;
pub mod matcher;
pub mod platform;
pub mod redact;
mod signal;
pub mod stats;
pub mod template;

pub use dispatcher::{Destination, WebhookDispatcher, WebhookDispatcherBuilder};
pub use error::WebhookError;
pub use hints::WithHints;
pub use limiter::{RateLimit, RetryPolicy};
pub use matcher::MatcherRegistry;
pub use platform::Platform;
pub use platform::{
    discord::{Discord, DiscordEmbed, EmbedField},
    generic::Generic,
    ntfy::Ntfy,
    slack::Slack,
    telegram::{ParseMode, Telegram},
};
pub use redact::redact_url;
pub use stats::DestinationStats;
pub use template::TemplateEngine;
