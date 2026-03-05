use serde_json::{Map, Value, json};

use super::Platform;

pub struct Telegram {
    token: String,
    chat_id: i64,
    parse_mode: ParseMode,
}

#[derive(Clone, Copy, Default)]
pub enum ParseMode {
    #[default]
    MarkdownV2,
    Html,
    Plain,
}

impl ParseMode {
    fn as_str(&self) -> Option<&'static str> {
        match self {
            Self::MarkdownV2 => Some("MarkdownV2"),
            Self::Html => Some("HTML"),
            Self::Plain => None,
        }
    }
}

impl Telegram {
    pub fn new(token: impl Into<String>, chat_id: i64) -> Self {
        Self {
            token: token.into(),
            chat_id,
            parse_mode: ParseMode::default(),
        }
    }

    pub fn with_parse_mode(mut self, parse_mode: ParseMode) -> Self {
        self.parse_mode = parse_mode;
        self
    }
}

/// Escapes all reserved MarkdownV2 characters as required by Telegram.
/// See: https://core.telegram.org/bots/api#markdownv2-style
fn escape_markdown_v2(text: &str) -> String {
    const RESERVED: &[char] = &[
        '_', '*', '[', ']', '(', ')', '~', '`', '>', '#', '+', '-', '=', '|', '{', '}', '.', '!',
    ];

    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if RESERVED.contains(&ch) {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

impl Platform for Telegram {
    fn build_payload(&self, rendered: &str, hints: &Map<String, Value>) -> Value {
        let text = match self.parse_mode {
            ParseMode::MarkdownV2 => escape_markdown_v2(rendered),
            _ => rendered.to_owned(),
        };

        let mut payload = json!({
            "chat_id": self.chat_id,
            "text": text,
        });

        if let Some(mode) = self.parse_mode.as_str() {
            payload["parse_mode"] = Value::String(mode.to_owned());
        }

        if hints
            .get("__tg_silent")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            payload["disable_notification"] = Value::Bool(true);
        }

        if hints
            .get("__tg_disable_preview")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            payload["disable_web_page_preview"] = Value::Bool(true);
        }

        payload
    }

    fn endpoint(&self) -> &str {
        // Telegram requires a static-lifetime string for endpoint() but we build
        // the URL dynamically, so we leak it once per Telegram instance.
        // This is acceptable since Telegram instances are created at startup and
        // live for the duration of the program.
        Box::leak(
            format!("https://api.telegram.org/bot{}/sendMessage", self.token).into_boxed_str(),
        )
    }
}
