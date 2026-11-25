use std::{fmt, time::Duration};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use super::Platform;
use crate::{limiter::RateLimit, redact::redact_url};

// Discord embed limits (characters, not bytes).
pub const MAX_TITLE: usize = 256;
pub const MAX_DESCRIPTION: usize = 4096;
pub const MAX_FIELDS: usize = 25;
pub const MAX_FIELD_NAME: usize = 256;
pub const MAX_FIELD_VALUE: usize = 1024;
pub const MAX_FOOTER: usize = 2048;
pub const MAX_AUTHOR: usize = 256;
/// Sum of all text in all embeds of one message.
pub const MAX_EMBED_TOTAL: usize = 6000;
pub const MAX_EMBEDS_PER_MESSAGE: usize = 10;

const ELLIPSIS: char = '…';
/// Discord rejects empty field names/values; a zero-width space is accepted.
const ZWSP: &str = "\u{200b}";

/// Truncates to at most `max` chars (never splitting a char), ending in `…`.
pub fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_owned();
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    if max > 0 {
        out.push(ELLIPSIS);
    }
    out
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EmbedField {
    pub name: String,
    pub value: String,
    #[serde(default)]
    pub inline: bool,
}

/// A Discord embed. Limits are enforced when it is turned into JSON
/// ([`DiscordEmbed::to_json`]), so builders never have to count characters.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DiscordEmbed {
    pub title: Option<String>,
    pub description: Option<String>,
    /// Only `https://` URLs are kept.
    pub url: Option<String>,
    pub color: Option<u32>,
    pub fields: Vec<EmbedField>,
    pub footer: Option<String>,
    /// ISO 8601, e.g. `2026-11-25T12:00:00Z`.
    pub timestamp: Option<String>,
    pub author: Option<String>,
}

impl DiscordEmbed {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn title(mut self, v: impl Into<String>) -> Self {
        self.title = Some(v.into());
        self
    }

    pub fn description(mut self, v: impl Into<String>) -> Self {
        self.description = Some(v.into());
        self
    }

    pub fn url(mut self, v: impl Into<String>) -> Self {
        self.url = Some(v.into());
        self
    }

    pub fn color(mut self, v: u32) -> Self {
        self.color = Some(v);
        self
    }

    pub fn field(
        mut self,
        name: impl Into<String>,
        value: impl Into<String>,
        inline: bool,
    ) -> Self {
        self.fields.push(EmbedField {
            name: name.into(),
            value: value.into(),
            inline,
        });
        self
    }

    pub fn footer(mut self, v: impl Into<String>) -> Self {
        self.footer = Some(v.into());
        self
    }

    pub fn timestamp(mut self, v: impl Into<String>) -> Self {
        self.timestamp = Some(v.into());
        self
    }

    pub fn author(mut self, v: impl Into<String>) -> Self {
        self.author = Some(v.into());
        self
    }

    /// Discord-format JSON with every limit enforced (truncating with `…`).
    pub fn to_json(&self) -> Value {
        let mut title = self.title.as_deref().map(|s| truncate_chars(s, MAX_TITLE));
        let mut description = self
            .description
            .as_deref()
            .map(|s| truncate_chars(s, MAX_DESCRIPTION));
        let footer = self
            .footer
            .as_deref()
            .map(|s| truncate_chars(s, MAX_FOOTER));
        let author = self
            .author
            .as_deref()
            .map(|s| truncate_chars(s, MAX_AUTHOR));

        let mut fields: Vec<(String, String, bool)> = self
            .fields
            .iter()
            .take(MAX_FIELDS)
            .map(|f| {
                let name = truncate_chars(&f.name, MAX_FIELD_NAME);
                let value = truncate_chars(&f.value, MAX_FIELD_VALUE);
                (
                    if name.is_empty() {
                        ZWSP.to_owned()
                    } else {
                        name
                    },
                    if value.is_empty() {
                        ZWSP.to_owned()
                    } else {
                        value
                    },
                    f.inline,
                )
            })
            .collect();

        let chars = |s: &Option<String>| s.as_deref().map_or(0, |s| s.chars().count());
        let total = |title: &Option<String>,
                     description: &Option<String>,
                     fields: &[(String, String, bool)]| {
            chars(title)
                + chars(description)
                + chars(&footer)
                + chars(&author)
                + fields
                    .iter()
                    .map(|(n, v, _)| n.chars().count() + v.chars().count())
                    .sum::<usize>()
        };

        // Enforce the 6000 total: shed fields from the end, then shrink the description.
        while total(&title, &description, &fields) > MAX_EMBED_TOTAL && !fields.is_empty() {
            fields.pop();
        }
        let overflow = total(&title, &description, &fields).saturating_sub(MAX_EMBED_TOTAL);
        if overflow > 0 {
            if let Some(d) = &description {
                let keep = d.chars().count().saturating_sub(overflow);
                description = Some(truncate_chars(d, keep));
            }
        }
        let overflow = total(&title, &description, &fields).saturating_sub(MAX_EMBED_TOTAL);
        if overflow > 0 {
            if let Some(t) = &title {
                let keep = t.chars().count().saturating_sub(overflow);
                title = Some(truncate_chars(t, keep));
            }
        }

        let mut embed = Map::new();
        if let Some(t) = title {
            embed.insert("title".into(), Value::String(t));
        }
        if let Some(d) = description {
            embed.insert("description".into(), Value::String(d));
        }
        if let Some(u) = self.url.as_deref().filter(|u| u.starts_with("https://")) {
            embed.insert("url".into(), Value::String(u.to_owned()));
        }
        if let Some(c) = self.color {
            embed.insert("color".into(), Value::from(c));
        }
        if !fields.is_empty() {
            let arr = fields
                .into_iter()
                .map(|(name, value, inline)| json!({ "name": name, "value": value, "inline": inline }))
                .collect();
            embed.insert("fields".into(), Value::Array(arr));
        }
        if let Some(f) = footer {
            embed.insert("footer".into(), json!({ "text": f }));
        }
        if let Some(ts) = &self.timestamp {
            embed.insert("timestamp".into(), Value::String(ts.clone()));
        }
        if let Some(a) = author {
            embed.insert("author".into(), json!({ "name": a }));
        }
        Value::Object(embed)
    }

    /// Reads the hint keys (`__d_*`) produced by `WithHints`, using `rendered`
    /// as the description unless an explicit embed/description hint provides one.
    fn from_hints(rendered: &str, hints: &Map<String, Value>) -> Self {
        let mut e: DiscordEmbed = hints
            .get("__d_embed")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        if e.description.is_none() {
            e.description = Some(rendered.to_owned());
        }
        let text = |k: &str| hints.get(k).and_then(Value::as_str).map(str::to_owned);
        if let Some(v) = hints.get("__d_color").and_then(Value::as_u64) {
            e.color = u32::try_from(v).ok();
        }
        e.title = text("__d_title").or(e.title);
        e.url = text("__d_url").or(e.url);
        e.footer = text("__d_footer").or(e.footer);
        e.timestamp = text("__d_timestamp").or(e.timestamp);
        e.author = text("__d_author").or(e.author);
        if let Some(Value::Array(fields)) = hints.get("__d_fields") {
            e.fields.extend(
                fields
                    .iter()
                    .filter_map(|f| serde_json::from_value::<EmbedField>(f.clone()).ok()),
            );
        }
        e
    }
}

fn embed_size(embed: &Value) -> usize {
    let s = |v: Option<&Value>| v.and_then(Value::as_str).map_or(0, |s| s.chars().count());
    let mut n = s(embed.get("title")) + s(embed.get("description"));
    n += s(embed.pointer("/footer/text")) + s(embed.pointer("/author/name"));
    if let Some(Value::Array(fields)) = embed.get("fields") {
        n += fields
            .iter()
            .map(|f| s(f.get("name")) + s(f.get("value")))
            .sum::<usize>();
    }
    n
}

pub struct Discord {
    endpoint: String,
    username: Option<String>,
    allowed_mentions: Value,
    limits: Vec<RateLimit>,
}

impl Discord {
    /// `webhook_url` is the usual `https://discord.com/api/webhooks/<id>/<token>`;
    /// it is posted to the versioned `/api/v10/` path.
    pub fn new(webhook_url: impl Into<String>) -> Self {
        let url: String = webhook_url.into();
        let endpoint = url.replacen("/api/webhooks/", "/api/v10/webhooks/", 1);
        Self {
            endpoint,
            username: None,
            // Never ping anyone from message text unless the caller opts in.
            allowed_mentions: json!({ "parse": [] }),
            // Global per-webhook budget and the short burst bucket.
            limits: vec![
                RateLimit::new(30, Duration::from_secs(60)),
                RateLimit::new(5, Duration::from_secs(2)),
            ],
        }
    }

    pub fn with_username(mut self, username: impl Into<String>) -> Self {
        self.username = Some(username.into());
        self
    }

    /// Post into a thread (`?thread_id=`).
    pub fn with_thread_id(mut self, thread_id: impl AsRef<str>) -> Self {
        let sep = if self.endpoint.contains('?') {
            '&'
        } else {
            '?'
        };
        self.endpoint
            .push_str(&format!("{sep}thread_id={}", thread_id.as_ref()));
        self
    }

    /// Replace the default `{"parse": []}` (which suppresses every mention).
    pub fn with_allowed_mentions(mut self, allowed_mentions: Value) -> Self {
        self.allowed_mentions = allowed_mentions;
        self
    }

    /// Override the built-in rate-limit buckets (30 per 60 s and 5 per 2 s).
    pub fn with_rate_limits(mut self, limits: Vec<RateLimit>) -> Self {
        self.limits = limits;
        self
    }
}

impl fmt::Debug for Discord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Discord")
            .field("endpoint", &redact_url(&self.endpoint))
            .field("username", &self.username)
            .finish_non_exhaustive()
    }
}

impl Platform for Discord {
    fn build_payload(&self, rendered: &str, hints: &Map<String, Value>) -> Value {
        let embed = DiscordEmbed::from_hints(rendered, hints).to_json();
        let mut payload = json!({
            "embeds": [embed],
            "allowed_mentions": self.allowed_mentions,
        });
        if let Some(username) = &self.username {
            payload["username"] = Value::String(username.clone());
        }
        payload
    }

    fn endpoint(&self) -> &str {
        &self.endpoint
    }

    fn rate_limits(&self) -> Vec<RateLimit> {
        self.limits.clone()
    }

    /// Up to 10 embeds (and 6000 chars) per message; whatever does not fit is
    /// summarised as `+k more` in the message content.
    fn coalesce(&self, batch: &[Value]) -> Option<Value> {
        let first = batch.first()?;
        let mut embeds = Vec::new();
        let mut size = 0;
        let mut total = 0usize;
        for embed in batch.iter().flat_map(|p| {
            p.get("embeds")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
        }) {
            total += 1;
            let s = embed_size(embed);
            if embeds.len() < MAX_EMBEDS_PER_MESSAGE
                && (embeds.is_empty() || size + s <= MAX_EMBED_TOTAL)
            {
                size += s;
                embeds.push(embed.clone());
            }
        }
        if embeds.is_empty() {
            return None;
        }
        let mut payload = first.clone();
        payload["embeds"] = Value::Array(embeds.clone());
        let more = total - embeds.len();
        if more > 0 {
            payload["content"] = Value::String(format!("+{more} more"));
        }
        Some(payload)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payload(d: &Discord, text: &str, hints: Map<String, Value>) -> Value {
        d.build_payload(text, &hints)
    }

    #[test]
    fn truncates_on_char_boundaries() {
        assert_eq!(truncate_chars("héllo", 10), "héllo");
        assert_eq!(truncate_chars("héllo wörld", 5), "héll…");
        assert_eq!(truncate_chars("日本語日本語", 3), "日本…");
    }

    #[test]
    fn payload_truncates_and_mutes_mentions() {
        let d = Discord::new("https://discord.com/api/webhooks/1/tok");
        let long = "@everyone ".repeat(1000);
        let p = payload(&d, &long, Map::new());
        assert_eq!(p["allowed_mentions"], json!({ "parse": [] }));
        let desc = p["embeds"][0]["description"].as_str().unwrap();
        assert_eq!(desc.chars().count(), MAX_DESCRIPTION);
        assert!(desc.ends_with('…'));
    }

    #[test]
    fn endpoint_uses_v10_and_thread() {
        let d = Discord::new("https://discord.com/api/webhooks/1/tok").with_thread_id("42");
        assert_eq!(
            d.endpoint(),
            "https://discord.com/api/v10/webhooks/1/tok?thread_id=42"
        );
        assert_eq!(
            d.redacted_endpoint(),
            "https://discord.com/api/v10/webhooks/1/***"
        );
        assert!(!format!("{d:?}").contains("tok"));
    }

    #[test]
    fn embed_limits_and_total() {
        let mut e = DiscordEmbed::new()
            .title("t".repeat(300))
            .description("d".repeat(5000))
            .url("http://insecure.example")
            .footer("f".repeat(3000));
        for i in 0..30 {
            e = e.field(format!("n{i}"), "v".repeat(2000), i % 2 == 0);
        }
        let j = e.to_json();
        assert!(j.get("url").is_none());
        assert_eq!(j["title"].as_str().unwrap().chars().count(), MAX_TITLE);
        assert_eq!(
            j["footer"]["text"].as_str().unwrap().chars().count(),
            MAX_FOOTER
        );
        assert!(j["fields"].as_array().map_or(0, Vec::len) <= MAX_FIELDS);
        assert!(embed_size(&j) <= MAX_EMBED_TOTAL, "size {}", embed_size(&j));
    }

    #[test]
    fn hints_build_full_embed() {
        let d = Discord::new("https://discord.com/api/webhooks/1/tok");
        let mut hints = Map::new();
        hints.insert("__d_title".into(), json!("T"));
        hints.insert("__d_url".into(), json!("https://example.com"));
        hints.insert("__d_footer".into(), json!("F"));
        hints.insert("__d_timestamp".into(), json!("2026-11-25T00:00:00Z"));
        hints.insert(
            "__d_fields".into(),
            json!([{ "name": "a", "value": "b", "inline": true }]),
        );
        let e = &payload(&d, "body", hints)["embeds"][0];
        assert_eq!(e["description"], "body");
        assert_eq!(e["footer"]["text"], "F");
        assert_eq!(e["fields"][0]["inline"], true);
        assert_eq!(e["timestamp"], "2026-11-25T00:00:00Z");
    }

    #[test]
    fn coalesce_caps_at_ten_embeds() {
        let d = Discord::new("https://discord.com/api/webhooks/1/tok");
        let batch: Vec<Value> = (0..14)
            .map(|i| payload(&d, &format!("m{i}"), Map::new()))
            .collect();
        let merged = d.coalesce(&batch).unwrap();
        assert_eq!(merged["embeds"].as_array().unwrap().len(), 10);
        assert_eq!(merged["content"], "+4 more");
        assert_eq!(merged["allowed_mentions"], json!({ "parse": [] }));
    }
}
