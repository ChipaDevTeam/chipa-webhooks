use serde_json::{Map, Value, json};

use super::Platform;

pub struct Generic {
    url: String,
    body_key: String,
}

impl Generic {
    /// Posts `{ "<body_key>": "<rendered>" }` to `url`.
    /// Default body key is `"text"`.
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            body_key: "text".to_owned(),
        }
    }

    pub fn with_body_key(mut self, key: impl Into<String>) -> Self {
        self.body_key = key.into();
        self
    }
}

impl Platform for Generic {
    fn build_payload(&self, rendered: &str, _hints: &Map<String, Value>) -> Value {
        json!({ self.body_key.as_str(): rendered })
    }

    fn endpoint(&self) -> &str {
        &self.url
    }
}
