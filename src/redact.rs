/// Redacts the secret parts of a webhook URL so it can be logged or shown in errors.
///
/// * Discord: `https://discord.com/api/v10/webhooks/<id>/***`
/// * Slack: `https://hooks.slack.com/services/***`
/// * Telegram: `https://api.telegram.org/bot***/sendMessage`
/// * anything else: `scheme://host/***`
///
/// Query strings and userinfo are always dropped.
pub fn redact_url(url: &str) -> String {
    let (scheme, rest) = match url.split_once("://") {
        Some((s, r)) => (s, r),
        None => return "***".to_owned(),
    };
    let rest = rest.split(['?', '#']).next().unwrap_or("");
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, ""),
    };
    let host = authority.rsplit('@').next().unwrap_or(authority);
    let base = format!("{scheme}://{host}");

    if let Some(i) = path.find("/webhooks/") {
        let after = &path[i + "/webhooks/".len()..];
        let id = after.split('/').next().unwrap_or("");
        return format!("{base}{}/webhooks/{id}/***", &path[..i]);
    }
    if path.starts_with("/services/") {
        return format!("{base}/services/***");
    }
    if let Some(after) = path.strip_prefix("/bot") {
        let tail = after.find('/').map(|j| &after[j..]).unwrap_or("");
        return format!("{base}/bot***{tail}");
    }
    format!("{base}/***")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discord() {
        assert_eq!(
            redact_url("https://discord.com/api/v10/webhooks/123/SECRET?thread_id=9"),
            "https://discord.com/api/v10/webhooks/123/***"
        );
    }

    #[test]
    fn slack_telegram_generic() {
        assert_eq!(
            redact_url("https://hooks.slack.com/services/T/B/SECRET"),
            "https://hooks.slack.com/services/***"
        );
        assert_eq!(
            redact_url("https://api.telegram.org/bot123:ABC/sendMessage"),
            "https://api.telegram.org/bot***/sendMessage"
        );
        assert_eq!(
            redact_url("https://user:pw@example.com/hook/abc?k=v"),
            "https://example.com/***"
        );
    }
}
