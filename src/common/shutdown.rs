use std::{
    fmt::Debug,
    future::Future,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use tokio::sync::Notify;

/// Thread-safe shutdown coordinator
#[derive(Clone)]
pub struct Shutdown {
    /// Tuple of (shutdown flag, notification mechanism)
    /// Both wrapped in Arc for thread-safe sharing
    inner: Arc<(AtomicBool, Notify)>,
}

impl Shutdown {
    /// Creates a new shutdown coordinator
    pub fn new() -> Self {
        Self {
            inner: Arc::new((AtomicBool::new(false), Notify::new())),
        }
    }

    /// Initiates shutdown
    pub fn shutdown(&self) {
        self.inner.0.swap(true, Ordering::Relaxed);
        self.inner.1.notify_waiters();
    }

    /// Resets the shutdown state
    pub fn reset(&self) {
        self.inner.0.store(false, Ordering::Relaxed);
    }

    /// Checks if shutdown has been initiated
    pub fn is_terminated(&self) -> bool {
        self.inner.0.load(Ordering::Relaxed)
    }

    /// Waits for shutdown to be initiated
    pub fn wait(&'_ self) -> impl Future<Output = ()> + Send + 'static {
        let inner = self.inner.clone();
        async move {
            // Initial fast check
            if !inner.0.load(Ordering::Relaxed) {
                let notify = inner.1.notified();
                // Second check to avoid "missed wakeup" race conditions
                if !inner.0.load(Ordering::Relaxed) {
                    notify.await;
                }
            }
        }
    }
}

impl Default for Shutdown {
    /// Creates a new shutdown coordinator with default settings
    fn default() -> Self {
        Self::new()
    }
}

impl Debug for Shutdown {
    /// Provides debug formatting for the shutdown coordinator
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        f.debug_struct("Shutdown").field("is_terminated", &self.inner.0.load(Ordering::Relaxed)).finish()
    }
}
