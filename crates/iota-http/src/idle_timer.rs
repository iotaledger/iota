// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! A timer that measures how long a task has had nothing to do.

use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Duration,
};

use parking_lot::Mutex;
use pin_project_lite::pin_project;
use tokio::time::{Instant, Sleep};

/// Creates a timer that becomes ready once its task has been idle for
/// `timeout`.
///
/// The task is idle whenever it holds no [`IdleGuard`]. It starts idle, so one
/// that never takes a guard becomes ready after a single timeout. Taking a
/// guard stops the timer, and dropping the last one starts it again from that
/// moment — so this measures the time since the task last had something to do,
/// not the time since it was created.
///
/// The work is generally not done by the task holding the timer, so guards are
/// taken through an [`IdleHandle`], which can be cloned and sent wherever the
/// work happens.
///
/// The timer must be pinned once and polled by reference. Building it inside a
/// `select!` branch would construct a new one on every pass of the loop, and a
/// loop that goes round more often than the timeout would never reach it.
///
/// The timer stays ready for as long as the task stays idle, so a loop must
/// stop polling it once it has acted on it.
///
/// ```ignore
/// let timer = idle_timeout(Duration::from_secs(300));
/// let handle = timer.handle();
///
/// // Wherever the work is: the task counts as busy until the guard is dropped.
/// let guard = handle.guard();
///
/// let mut timer = std::pin::pin!(timer);
/// loop {
///     tokio::select! {
///         _ = &mut timer => {
///             // … wind the task down, and stop polling the timer
///         }
///         // … whatever else the task is waiting for
///     }
/// }
/// ```
pub fn idle_timeout(timeout: Duration) -> IdleTimer {
    let now = Instant::now();
    IdleTimer {
        sleep: tokio::time::sleep_until(now + timeout),
        timeout,
        state: Arc::new(Mutex::new(State {
            busy: 0,
            idle_since: now,
        })),
    }
}

pin_project! {
    /// A timer that runs only while its task is idle. See [`idle_timeout`].
    pub struct IdleTimer {
        // Sleep timer, never set later than the moment the task will have been
        // idle for timeout; when it fires the deadline is worked out again.
        #[pin]
        sleep: Sleep,
        // Idle timeout: period after which the sleep timer becomes ready.
        timeout: Duration,
        // Whether the task is busy and when it last became idle, shared with
        // the handles and guards that mark it busy.
        state: Arc<Mutex<State>>,
    }
}

impl IdleTimer {
    /// A handle for marking the task busy, which can be cloned and sent to
    /// wherever its work is done.
    pub fn handle(&self) -> IdleHandle {
        IdleHandle {
            state: self.state.clone(),
        }
    }
}

impl Future for IdleTimer {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut timer = self.project();

        // Guards do not wake the timer. Taking or dropping one can only move
        // the moment the task will have been idle for `timeout` later, so the
        // sleep is left at the earlier deadline and the deadline is worked out
        // again when it fires. The sleep is polled again after a reset, which
        // is what registers the waker for the new deadline.
        while timer.sleep.as_mut().poll(cx).is_ready() {
            let now = Instant::now();
            let deadline = {
                let state = timer.state.lock();
                if state.busy == 0 {
                    state.idle_since + *timer.timeout
                } else {
                    now + *timer.timeout
                }
            };
            if deadline <= now {
                return Poll::Ready(());
            }
            timer.sleep.as_mut().reset(deadline);
        }
        Poll::Pending
    }
}

/// Marks a task busy for as long as the guards it hands out are held. See
/// [`idle_timeout`].
#[derive(Clone, Debug)]
pub struct IdleHandle {
    state: Arc<Mutex<State>>,
}

impl IdleHandle {
    /// Counts the task as busy until the returned guard is dropped.
    pub fn guard(&self) -> IdleGuard {
        self.state.lock().busy += 1;
        IdleGuard {
            state: self.state.clone(),
        }
    }
}

/// Counts its task as busy for as long as it is held. See [`idle_timeout`].
#[must_use = "the task is only counted busy for as long as the guard is held"]
#[derive(Debug)]
pub struct IdleGuard {
    state: Arc<Mutex<State>>,
}

impl Drop for IdleGuard {
    fn drop(&mut self) {
        let mut state = self.state.lock();
        state.busy -= 1;
        if state.busy == 0 {
            // Read under the lock, so a later idle period never stores an
            // earlier moment than the one before it.
            state.idle_since = Instant::now();
        }
    }
}

#[derive(Debug)]
struct State {
    /// Live [`IdleGuard`]s; the task is idle when this is zero.
    busy: usize,
    /// When `busy` last fell to zero, or when the timer was created if it
    /// never has. Read only while `busy` is zero.
    idle_since: Instant,
}
