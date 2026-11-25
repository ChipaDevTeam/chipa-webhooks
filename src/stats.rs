use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

/// Snapshot of one destination's counters.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DestinationStats {
    /// Payloads delivered (including ones merged into a coalesced message).
    pub sent: u64,
    /// Payloads that could not be delivered after all retries.
    pub failed: u64,
    /// Payloads dropped because a queue was full.
    pub dropped: u64,
    /// Retry attempts (429 / 5xx) made.
    pub retried: u64,
    /// Payloads that were merged into another message by coalescing
    /// (already counted in `sent`).
    pub coalesced: u64,
}

#[derive(Default)]
pub(crate) struct Counters {
    pub(crate) sent: AtomicU64,
    pub(crate) failed: AtomicU64,
    pub(crate) dropped: AtomicU64,
    pub(crate) retried: AtomicU64,
    pub(crate) coalesced: AtomicU64,
}

impl Counters {
    pub(crate) fn add(field: &AtomicU64, n: u64) {
        field.fetch_add(n, Relaxed);
    }

    pub(crate) fn snapshot(&self) -> DestinationStats {
        DestinationStats {
            sent: self.sent.load(Relaxed),
            failed: self.failed.load(Relaxed),
            dropped: self.dropped.load(Relaxed),
            retried: self.retried.load(Relaxed),
            coalesced: self.coalesced.load(Relaxed),
        }
    }
}
