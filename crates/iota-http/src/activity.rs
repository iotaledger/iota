// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! A timer that measures how long a task has had nothing to do.

use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
    time::Duration,
};

use pin_project_lite::pin_project;
use tokio::time::{Instant, Sleep};

/// Far enough ahead that a timer set for it is never reached. Only used to
/// give the timer of a task with no deadline some value; it is never polled.
const NEVER: Duration = Duration::from_secs(60 * 60 * 24 * 365 * 30);

/// Creates a timer that becomes ready once its task has been idle for
/// `idle_timeout`, or one that is never ready when there is no timeout.
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
/// ```ignore
/// let timer = idle_sleep(Some(Duration::from_secs(300)));
/// let handle = timer.handle();
///
/// // Wherever the work is: the task counts as busy until the guard is dropped.
/// let guard = handle.guard();
///
/// let mut timer = std::pin::pin!(timer);
/// loop {
///     tokio::select! {
///         _ = &mut timer => {
///             // Work can arrive between the timer being reached and this
///             // running, so readiness alone does not mean it is still idle.
///             if timer.is_busy() {
///                 continue;
///             }
///             break;
///         }
///         // … whatever else the task is waiting for
///     }
/// }
/// ```
pub fn idle_sleep(idle_timeout: Option<Duration>) -> IdleSleep {
    IdleSleep {
        sleep: tokio::time::sleep(idle_timeout.unwrap_or(NEVER)),
        timeout: idle_timeout,
        shared: Arc::default(),
    }
}

pin_project! {
    /// A timer that runs only while its task is idle. See [`idle_sleep`].
    pub struct IdleSleep {
        #[pin]
        sleep: Sleep,
        // `None` for a task that is never to be closed for being idle.
        timeout: Option<Duration>,
        shared: Arc<Mutex<Shared>>,
    }
}

impl IdleSleep {
    /// A handle for marking the task busy, which can be cloned and sent to
    /// wherever its work is done.
    pub fn handle(&self) -> IdleHandle {
        IdleHandle {
            shared: self.shared.clone(),
        }
    }

    /// Whether the task has anything to do right now.
    pub fn is_busy(&self) -> bool {
        self.shared.lock().unwrap().busy > 0
    }
}

impl std::fmt::Debug for IdleSleep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IdleSleep")
            .field("timeout", &self.timeout)
            .finish_non_exhaustive()
    }
}

/// Marks a task busy for as long as the guards it hands out are held. See
/// [`idle_sleep`].
#[derive(Clone, Debug)]
pub struct IdleHandle {
    shared: Arc<Mutex<Shared>>,
}

impl IdleHandle {
    /// Counts the task as busy until the returned guard is dropped.
    pub fn guard(&self) -> IdleGuard {
        self.shared.lock().unwrap().busy += 1;
        IdleGuard {
            shared: self.shared.clone(),
        }
    }

    /// Whether the task has anything to do right now.
    pub fn is_busy(&self) -> bool {
        self.shared.lock().unwrap().busy > 0
    }

    /// When the task last became idle, or `None` if it has never been busy.
    ///
    /// Unlike the timer, this is not consumed when the timer restarts, so it
    /// can be read at any time to order tasks by how long each has had nothing
    /// to do.
    pub fn idle_since(&self) -> Option<std::time::Instant> {
        self.shared
            .lock()
            .unwrap()
            .idle_since
            .map(Instant::into_std)
    }
}

/// Counts its task as busy for as long as it is held. See [`idle_sleep`].
#[must_use = "the task is only counted busy for as long as the guard is held"]
#[derive(Debug)]
pub struct IdleGuard {
    shared: Arc<Mutex<Shared>>,
}

impl Drop for IdleGuard {
    fn drop(&mut self) {
        let mut shared = self.shared.lock().unwrap();
        debug_assert!(
            shared.busy > 0,
            "guards are only ever created by IdleHandle::guard, which counts them"
        );
        shared.busy -= 1;
        if shared.busy == 0 {
            // The last of the task's work is done: the timer restarts from
            // here, and the task is woken to do it.
            shared.idle_since = Some(Instant::now());
            shared.restart_pending = true;
            if let Some(waker) = shared.waker.take() {
                waker.wake();
            }
        }
    }
}

#[derive(Default, Debug)]
struct Shared {
    /// Live [`IdleGuard`]s. The task is idle when this is zero.
    busy: usize,
    /// When the task last became idle, or `None` if it has never been busy.
    idle_since: Option<Instant>,
    /// Set when `idle_since` has moved and the timer has yet to be restarted
    /// from it. Cleared by the poll that does so.
    restart_pending: bool,
    /// Waker of the task polling the timer, woken when the task becomes idle.
    waker: Option<Waker>,
}

impl Future for IdleSleep {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();

        let Some(timeout) = *this.timeout else {
            // No deadline, so nothing ever makes this ready and nothing needs
            // to wake the task on its account.
            return Poll::Pending;
        };

        let mut shared = this.shared.lock().unwrap();

        if shared.busy > 0 {
            // The task has work, so the timer does not run. Dropping the last
            // guard wakes us to start it again.
            shared.waker = Some(cx.waker().clone());
            return Poll::Pending;
        }

        if shared.restart_pending {
            shared.restart_pending = false;
            let idle_since = shared
                .idle_since
                .expect("a restart is only ever pending once the task has become idle");
            this.sleep.as_mut().reset(idle_since + timeout);
        }

        // Not held across the poll below: nothing there touches the shared
        // state, and a guard taken meanwhile is dealt with on the next poll.
        drop(shared);

        this.sleep.poll(cx)
    }
}
