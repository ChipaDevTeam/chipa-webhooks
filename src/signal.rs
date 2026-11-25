use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use tokio::sync::Notify;

/// One-shot completion signal that can be awaited from async code or blocked on
/// from a plain thread.
pub(crate) struct Signal {
    done: Mutex<bool>,
    cv: Condvar,
    notify: Notify,
}

impl Signal {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self {
            done: Mutex::new(false),
            cv: Condvar::new(),
            notify: Notify::new(),
        })
    }

    pub(crate) fn fire(&self) {
        *self.done.lock().unwrap() = true;
        self.cv.notify_all();
        // Stores a permit when nobody is waiting yet, so no wakeup is lost.
        self.notify.notify_one();
    }

    pub(crate) async fn wait(&self) {
        loop {
            if *self.done.lock().unwrap() {
                return;
            }
            self.notify.notified().await;
        }
    }

    /// Returns `true` if the signal fired before `timeout` elapsed.
    pub(crate) fn wait_timeout(&self, timeout: Duration) -> bool {
        let guard = self.done.lock().unwrap();
        let (guard, _) = self
            .cv
            .wait_timeout_while(guard, timeout, |done| !*done)
            .unwrap();
        *guard
    }
}
