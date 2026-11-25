use serde_json::{Map, Value};

use crate::{limiter::RateLimit, redact::redact_url};

pub mod discord;
pub mod generic;
pub mod ntfy;
pub mod slack;
pub mod telegram;

pub trait Platform: Send + Sync {
    fn build_payload(&self, rendered: &str, hints: &Map<String, Value>) -> Value;
    fn endpoint(&self) -> &str;

    /// Rate-limit buckets enforced by the destination's worker. Default: none.
    fn rate_limits(&self) -> Vec<RateLimit> {
        Vec::new()
    }

    /// Merge several queued payloads into one message. Called only when the
    /// worker had to wait on a rate-limit bucket and more than one payload is
    /// queued. Return `None` (default) to deliver them one by one instead.
    fn coalesce(&self, _batch: &[Value]) -> Option<Value> {
        None
    }

    /// The endpoint with secrets removed; safe to log.
    fn redacted_endpoint(&self) -> String {
        redact_url(self.endpoint())
    }
}
