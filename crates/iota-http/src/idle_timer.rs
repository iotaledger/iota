// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! A timer that measures how long a task has had nothing to do.

use std::{sync::Arc, time::Duration};

use parking_lot::Mutex;
use tokio::time::Instant;

/// Creates a timer whose [`IdleTimer::wait`] completes once its task has been
/// idle for `timeout`.
///
/// The task is idle whenever it holds no [`IdleGuard`]. It starts idle, so for
/// one that never takes a guard `wait` completes after a single timeout. Taking
/// a guard stops the timer, and dropping the last one starts it again from that
/// moment — so this measures the time since the task last had something to do,
/// not the time since it was created.
///
/// The work is generally not done by the task holding the timer, so guards are
/// taken through an [`IdleHandle`], which can be cloned and sent wherever the
/// work happens.
///
/// [`IdleTimer::wait`] completes at once while the task has already been idle
/// for `timeout`, so a loop must stop waiting once it has acted on it.
///
/// ```ignore
/// let timer = idle_timeout(Duration::from_secs(300));
/// let handle = timer.handle();
///
/// // Wherever the work is: the task counts as busy until the guard is dropped.
/// let guard = handle.guard();
///
/// let mut wait = std::pin::pin!(timer.wait());
/// loop {
///     tokio::select! {
///         _ = &mut wait => {
///             // … wind the task down, and stop polling `wait`
///         }
///         // … whatever else the task is waiting for
///     }
/// }
/// ```
pub fn idle_timeout(timeout: Duration) -> IdleTimer {
    IdleTimer {
        timeout,
        state: Arc::new(Mutex::new(State {
            counter: 0,
            idle_since: Instant::now(),
        })),
    }
}

/// A timer that runs only while its task is idle. See [`idle_timeout`].
pub struct IdleTimer {
    // Idle timeout: period after which `wait` completes.
    timeout: Duration,
    // Whether the task is busy and when it last became idle, shared with
    // the handles and guards that mark it busy.
    state: Arc<Mutex<State>>,
}

impl IdleTimer {
    /// A handle for marking the task busy, which can be cloned and sent to
    /// wherever its work is done.
    pub fn handle(&self) -> IdleHandle {
        IdleHandle {
            state: self.state.clone(),
        }
    }

    /// Completes once the task has been idle for `timeout`, at once if it
    /// already has.
    pub async fn wait(&self) {
        // Guards do not wake the timer. Taking or dropping one can only move
        // the moment the task will have been idle for `timeout` later, so this
        // sleeps until the earliest such moment and works it out again when it
        // wakes.
        loop {
            let now = Instant::now();
            let deadline = {
                let state = self.state.lock();
                if state.counter == 0 {
                    state.idle_since + self.timeout
                } else {
                    now + self.timeout
                }
            };
            if deadline <= now {
                return;
            }
            tokio::time::sleep_until(deadline).await;
        }
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
        self.state.lock().counter += 1;
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
        state.counter -= 1;
        if state.counter == 0 {
            // Read under the lock, so a later idle period never stores an
            // earlier moment than the one before it.
            state.idle_since = Instant::now();
        }
    }
}

#[derive(Debug)]
struct State {
    /// Live [`IdleGuard`]s; the task is idle when this is zero.
    counter: usize,
    /// When `counter` last fell to zero, or when the timer was created if it
    /// never has. Read only while `counter` is zero.
    idle_since: Instant,
}
