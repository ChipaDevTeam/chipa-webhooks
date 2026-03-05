use serde_json::{Map, Value};

pub struct WithHints {
    pub(crate) map: Map<String, Value>,
}

impl WithHints {
    pub fn new() -> Self {
        Self { map: Map::new() }
    }

    pub fn d_color(mut self, color: u32) -> Self {
        self.map
            .insert("__d_color".to_owned(), Value::Number(color.into()));
        self
    }

    pub fn d_title(mut self, title: impl Into<String>) -> Self {
        self.map
            .insert("__d_title".to_owned(), Value::String(title.into()));
        self
    }

    pub fn tg_silent(mut self) -> Self {
        self.map.insert("__tg_silent".to_owned(), Value::Bool(true));
        self
    }

    pub fn tg_disable_preview(mut self) -> Self {
        self.map
            .insert("__tg_disable_preview".to_owned(), Value::Bool(true));
        self
    }
}

impl Default for WithHints {
    fn default() -> Self {
        Self::new()
    }
}

/// Extracts and removes all `__`-prefixed hint keys from a Value::Object.
/// Returns the hints as a separate map. Non-object values return empty hints.
pub fn extract_hints(value: &mut Value) -> Map<String, Value> {
    let Some(map) = value.as_object_mut() else {
        return Map::new();
    };

    let hint_keys: Vec<String> = map
        .keys()
        .filter(|k| k.starts_with("__"))
        .cloned()
        .collect();

    let mut hints = Map::new();
    for key in hint_keys {
        if let Some(v) = map.remove(&key) {
            hints.insert(key, v);
        }
    }

    hints
}
