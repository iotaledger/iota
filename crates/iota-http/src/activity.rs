// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! A timer that measures how long a task has had nothing to do.

use std::{
    future::Future,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
    task::{Context, Poll, Waker},
    time::Duration,
};

use pin_project_lite::pin_project;
use tokio::time::{Instant, Sleep};

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
        timer: idle_timeout.map(|timeout| Timer {
            sleep: tokio::time::sleep(timeout),
            timeout,
        }),
        shared: Arc::new(Shared::new()),
    }
}

pin_project! {
    /// A timer that runs only while its task is idle. See [`idle_sleep`].
    pub struct IdleSleep {
        // Sleep timer, created only when finite idle timeout is requested.
        #[pin]
        timer: Option<Timer>,
        shared: Arc<Shared>,
    }
}

pin_project! {
    struct Timer {
        // Sleep timer, becomes ready only when the task has been idle for timeout or longer
        // (after the start or the last job was complete, ie. the last related IdleGuard was dropped).
        #[pin]
        sleep: Sleep,
        // Idle timeout: period after which the sleep timer becomes ready.
        timeout: Duration,
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
        self.shared.is_busy()
    }
}

/// Marks a task busy for as long as the guards it hands out are held. See
/// [`idle_sleep`].
#[derive(Clone, Debug)]
pub struct IdleHandle {
    shared: Arc<Shared>,
}

impl IdleHandle {
    /// Counts the task as busy until the returned guard is dropped.
    pub fn guard(&self) -> IdleGuard {
        self.shared.busy.fetch_add(1, Ordering::Relaxed);
        IdleGuard {
            shared: self.shared.clone(),
        }
    }

    #[cfg(test)]
    pub fn is_busy(&self) -> bool {
        self.shared.is_busy()
    }

    /// How long the task has had nothing to do, or `None` if it is busy.
    ///
    /// The operation is not atomic -- it may become busy right after it returns
    /// `Some`.
    pub fn idle_for(&self) -> Option<Duration> {
        (!self.shared.is_busy()).then(|| self.shared.idle_for())
    }
}

/// Counts its task as busy for as long as it is held. See [`idle_sleep`].
#[must_use = "the task is only counted busy for as long as the guard is held"]
#[derive(Debug)]
pub struct IdleGuard {
    shared: Arc<Shared>,
}

impl Drop for IdleGuard {
    fn drop(&mut self) {
        let previously_busy = self.shared.busy.fetch_sub(1, Ordering::Relaxed);
        debug_assert!(
            previously_busy > 0,
            "guards are only ever created by IdleHandle::guard, which counts them"
        );
        if previously_busy != 1 {
            return;
        }

        // The last of the task's work is done: the timer restarts from here,
        // and the task is woken to do it.
        let mut state = self.shared.state.lock().unwrap();
        let idle_since = Instant::now() - self.shared.created_at;
        self.shared
            .idle_since_ms
            .store(idle_since.as_millis() as u64, Ordering::Relaxed);
        state.restart_pending = true;
        if let Some(waker) = state.waker.take() {
            waker.wake();
        }
    }
}

#[derive(Debug)]
struct Shared {
    /// Live [`IdleGuard`]s; the task is idle when this is zero.
    ///
    /// Outside the lock because every request takes and drops a guard, and
    /// every connection is asked whether it is busy when a full listener looks
    /// for one to give up. The lock is still taken whenever this reaches zero,
    /// so the timer is armed in step with it.
    busy: AtomicUsize,
    /// Instance when the timer was created.
    created_at: Instant,
    /// When the task last became idle, in milliseconds relative to `start`.
    idle_since_ms: AtomicU64,
    state: Mutex<State>,
}

impl Shared {
    fn new() -> Self {
        Self {
            busy: AtomicUsize::new(0),
            created_at: Instant::now(),
            idle_since_ms: AtomicU64::new(0),
            state: Mutex::default(),
        }
    }

    fn is_busy(&self) -> bool {
        self.busy.load(Ordering::Relaxed) > 0
    }

    fn idle_since(&self) -> Instant {
        self.created_at + Duration::from_millis(self.idle_since_ms.load(Ordering::Relaxed))
    }

    fn idle_for(&self) -> Duration {
        self.idle_since().elapsed()
    }
}

#[derive(Default, Debug)]
struct State {
    /// Set when `idle_since` has moved and the timer has yet to be restarted
    /// from it. While it is false the timer is armed from the current
    /// `idle_since`, so a timer that is reached is genuinely due.
    restart_pending: bool,
    /// Waker of the task polling the timer, woken when the task becomes idle.
    waker: Option<Waker>,
}

impl Future for IdleSleep {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        let Some(timer) = this.timer.as_pin_mut() else {
            // No deadline, so nothing ever makes this ready and nothing needs
            // to wake the task on its account.
            return Poll::Pending;
        };

        let mut state = this.shared.state.lock().unwrap();

        // Registering the waker under the same lock the last guard takes is
        // what stops a wake going missing: either the guard has yet to reach
        // the lock and will find the waker there, or it has already been
        // through and this reads zero below.
        if this.shared.is_busy() {
            state.waker = Some(cx.waker().clone());
            return Poll::Pending;
        }

        let mut timer = timer.project();
        if state.restart_pending {
            state.restart_pending = false;
            let deadline = this.shared.idle_since() + *timer.timeout;
            timer.sleep.as_mut().reset(deadline);
        }

        // A guard dropped between the read above and the lock being taken
        // leaves the timer armed from the previous idle moment, which is never
        // later than the current one. It is reached early, and that poll
        // applies the restart rather than reporting the task idle.
        drop(state);

        timer.sleep.poll(cx)
    }
}
