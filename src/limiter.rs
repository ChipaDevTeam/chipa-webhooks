use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// "At most `max` requests per `per` window."
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RateLimit {
    pub max: u32,
    pub per: Duration,
}

impl RateLimit {
    pub const fn new(max: u32, per: Duration) -> Self {
        Self { max, per }
    }
}

/// Retry behaviour for 429 and 5xx responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryPolicy {
    /// Total attempts per payload (1 = never retry).
    pub max_attempts: u32,
    /// First backoff for 5xx (doubles each attempt).
    pub base_backoff: Duration,
    /// Upper bound for every single wait (including `Retry-After`).
    pub max_wait: Duration,
}

impl RetryPolicy {
    pub const fn none() -> Self {
        Self {
            max_attempts: 1,
            base_backoff: Duration::from_millis(500),
            max_wait: Duration::from_secs(60),
        }
    }

    pub(crate) fn backoff(&self, attempt: u32) -> Duration {
        let factor = 1u32
            .checked_shl(attempt.saturating_sub(1))
            .unwrap_or(u32::MAX);
        self.base_backoff.saturating_mul(factor).min(self.max_wait)
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            ..Self::none()
        }
    }
}

/// Sliding-window limiter over several buckets, plus a server-imposed block.
pub(crate) struct Limiter {
    buckets: Vec<(RateLimit, VecDeque<Instant>)>,
    blocked_until: Option<Instant>,
}

impl Limiter {
    pub(crate) fn new(limits: Vec<RateLimit>) -> Self {
        Self {
            buckets: limits
                .into_iter()
                .filter(|l| l.max > 0)
                .map(|l| (l, VecDeque::new()))
                .collect(),
            blocked_until: None,
        }
    }

    /// How long until a request may be sent.
    pub(crate) fn wait_time(&mut self, now: Instant) -> Duration {
        let mut wait = Duration::ZERO;
        if let Some(t) = self.blocked_until {
            if t > now {
                wait = t - now;
            } else {
                self.blocked_until = None;
            }
        }
        for (limit, hits) in &mut self.buckets {
            while hits
                .front()
                .is_some_and(|t| now.duration_since(*t) >= limit.per)
            {
                hits.pop_front();
            }
            if hits.len() >= limit.max as usize {
                // The hit `max` positions from the newest must expire first.
                let idx = hits.len() - limit.max as usize;
                let free_at = hits[idx] + limit.per;
                wait = wait.max(free_at.saturating_duration_since(now));
            }
        }
        wait
    }

    pub(crate) fn record(&mut self, now: Instant) {
        for (_, hits) in &mut self.buckets {
            hits.push_back(now);
        }
    }

    pub(crate) fn block_for(&mut self, now: Instant, d: Duration) {
        let t = now + d;
        if self.blocked_until.is_none_or(|b| b < t) {
            self.blocked_until = Some(t);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bucket_fills_then_waits_until_oldest_expires() {
        let mut l = Limiter::new(vec![RateLimit::new(2, Duration::from_secs(10))]);
        let t0 = Instant::now();
        assert_eq!(l.wait_time(t0), Duration::ZERO);
        l.record(t0);
        l.record(t0 + Duration::from_secs(1));
        let w = l.wait_time(t0 + Duration::from_secs(2));
        assert_eq!(w, Duration::from_secs(8));
        // The first hit expired at t0+10, freeing a slot.
        assert_eq!(l.wait_time(t0 + Duration::from_secs(10)), Duration::ZERO);
        l.record(t0 + Duration::from_secs(10));
        assert_eq!(
            l.wait_time(t0 + Duration::from_secs(10)),
            Duration::from_secs(1)
        );
    }

    #[test]
    fn block_for_extends_wait() {
        let mut l = Limiter::new(vec![]);
        let t0 = Instant::now();
        l.block_for(t0, Duration::from_secs(3));
        assert_eq!(
            l.wait_time(t0 + Duration::from_secs(1)),
            Duration::from_secs(2)
        );
        assert_eq!(l.wait_time(t0 + Duration::from_secs(4)), Duration::ZERO);
    }

    #[test]
    fn backoff_doubles_and_clamps() {
        let p = RetryPolicy {
            max_attempts: 5,
            base_backoff: Duration::from_secs(1),
            max_wait: Duration::from_secs(3),
        };
        assert_eq!(p.backoff(1), Duration::from_secs(1));
        assert_eq!(p.backoff(2), Duration::from_secs(2));
        assert_eq!(p.backoff(3), Duration::from_secs(3));
    }
}
