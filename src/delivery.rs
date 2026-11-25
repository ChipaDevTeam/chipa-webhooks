use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use reqwest::{Response, StatusCode, header::HeaderMap};
use serde_json::Value;

use crate::{
    error::WebhookError,
    limiter::{Limiter, RetryPolicy},
    platform::Platform,
    stats::Counters,
};

const BODY_EXCERPT_CHARS: usize = 200;

/// Sleeps until the limiter allows a request, then records it.
async fn acquire(limiter: &mut Limiter) {
    loop {
        let wait = limiter.wait_time(Instant::now());
        if wait.is_zero() {
            limiter.record(Instant::now());
            return;
        }
        tokio::time::sleep(wait).await;
    }
}

fn header_f64(headers: &HeaderMap, name: &str) -> Option<f64> {
    headers
        .get(name)?
        .to_str()
        .ok()?
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite() && *v >= 0.0)
}

fn secs(v: f64, max: Duration) -> Duration {
    Duration::try_from_secs_f64(v).unwrap_or(max).min(max)
}

/// Learns from `X-RateLimit-Remaining` / `X-RateLimit-Reset-After`: when the
/// bucket is exhausted, hold the next request until it resets.
fn observe_headers(headers: &HeaderMap, limiter: &mut Limiter, policy: &RetryPolicy) {
    if header_f64(headers, "x-ratelimit-remaining") == Some(0.0) {
        if let Some(reset) = header_f64(headers, "x-ratelimit-reset-after") {
            limiter.block_for(Instant::now(), secs(reset, policy.max_wait));
        }
    }
}

fn excerpt(body: &str) -> String {
    let mut out: String = body.chars().take(BODY_EXCERPT_CHARS).collect();
    if body.chars().count() > BODY_EXCERPT_CHARS {
        out.push('…');
    }
    out
}

async fn read_body(response: Response) -> String {
    response.text().await.unwrap_or_default()
}

/// Posts one payload, honouring the limiter and retrying 429 / 5xx.
pub(crate) async fn deliver(
    client: &reqwest::Client,
    platform: &dyn Platform,
    payload: &Value,
    destination: &str,
    limiter: &mut Limiter,
    policy: &RetryPolicy,
    counters: &Arc<Counters>,
) -> Result<(), WebhookError> {
    let max_attempts = policy.max_attempts.max(1);
    let mut attempt = 0;

    loop {
        attempt += 1;
        acquire(limiter).await;

        let response = client
            .post(platform.endpoint())
            .json(payload)
            .send()
            .await
            .map_err(|e| WebhookError::Http {
                destination: destination.to_owned(),
                source: e.without_url(),
            })?;

        let status = response.status();
        observe_headers(response.headers(), limiter, policy);
        if status.is_success() {
            return Ok(());
        }

        let retry_after_header = header_f64(response.headers(), "retry-after");
        let body = read_body(response).await;

        let wait = if status == StatusCode::TOO_MANY_REQUESTS {
            let from_body = || {
                serde_json::from_str::<Value>(&body)
                    .ok()?
                    .get("retry_after")?
                    .as_f64()
                    .filter(|v| v.is_finite() && *v >= 0.0)
            };
            Some(
                retry_after_header
                    .or_else(from_body)
                    .map(|v| secs(v, policy.max_wait))
                    .unwrap_or_else(|| policy.backoff(attempt)),
            )
        } else if status.is_server_error() {
            Some(policy.backoff(attempt))
        } else {
            None
        };

        match wait {
            Some(wait) if attempt < max_attempts => {
                Counters::add(&counters.retried, 1);
                limiter.block_for(Instant::now(), wait);
                // `acquire` at the top of the loop sleeps out the block.
            }
            _ => {
                return Err(WebhookError::Status {
                    destination: destination.to_owned(),
                    status: status.as_u16(),
                    body_excerpt: excerpt(&body),
                });
            }
        }
    }
}
