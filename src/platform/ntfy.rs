use serde_json::{Map, Value, json};

use super::Platform;

pub struct Ntfy {
    url: String,
    topic: String,
    title: Option<String>,
    priority: Option<u8>,
    tags: Vec<String>,
}

impl Ntfy {
    /// `url` is the ntfy server base URL, e.g. `"https://ntfy.sh"` or a self-hosted instance.
    /// `topic` is the topic to publish to, e.g. `"trading-alerts"`.
    pub fn new(url: impl Into<String>, topic: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            topic: topic.into(),
            title: None,
            priority: None,
            tags: Vec::new(),
        }
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Priority: 1 (min) to 5 (max). Default is 3 (default).
    pub fn with_priority(mut self, priority: u8) -> Self {
        self.priority = Some(priority.clamp(1, 5));
        self
    }

    pub fn with_tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tags = tags.into_iter().map(Into::into).collect();
        self
    }
}

impl Platform for Ntfy {
    fn build_payload(&self, rendered: &str, hints: &Map<String, Value>) -> Value {
        let title = hints
            .get("__ntfy_title")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .or_else(|| self.title.clone());

        let priority = hints
            .get("__ntfy_priority")
            .and_then(Value::as_u64)
            .map(|p| (p as u8).clamp(1, 5))
            .or(self.priority);

        // Hints can supply extra tags as a comma-separated string on top of the
        // default tags registered on the struct.
        let mut tags = self.tags.clone();
        if let Some(extra) = hints.get("__ntfy_tags").and_then(Value::as_str) {
            tags.extend(extra.split(',').map(|t| t.trim().to_owned()));
        }

        let mut payload = json!({
            "topic":   self.topic,
            "message": rendered,
        });

        if let Some(t) = title {
            payload["title"] = Value::String(t);
        }

        if let Some(p) = priority {
            payload["priority"] = Value::Number(p.into());
        }

        if !tags.is_empty() {
            payload["tags"] = Value::Array(tags.into_iter().map(Value::String).collect());
        }

        payload
    }

    fn endpoint(&self) -> &str {
        // ntfy REST endpoint is just the base URL — the topic is in the payload body.
        // We leak once at startup since Ntfy instances live for the program lifetime.
        Box::leak(format!("{}/", self.url.trim_end_matches('/')).into_boxed_str())
    }
}
