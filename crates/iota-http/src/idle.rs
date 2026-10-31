// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! A timer that measures how long a task has had nothing to do.

use std::{
    future::Future,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
    task::{Context, Poll, Waker},
    time::Duration,
};

use pin_project_lite::pin_project;
use tokio::time::{Instant, Sleep};

/// Creates a timer that becomes ready once its task has been idle for
/// `idle_timeout`, or one that is never ready when there is no timeout.
///
/// Only a timer with a timeout tracks its task. Without one, guards taken
/// through its handle count nothing, and [`IdleHandle::idle_for`] reports
/// `None`.
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
/// The timer is ready once each time the task goes idle. Once reached, it
/// stays pending until a guard has been taken and dropped again, however
/// briefly, so a loop can go on polling it after acting on it.
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
///             if timer.is_busy() == Some(true) {
///                 continue;
///             }
///             // … wind the task down
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
            shared: Arc::new(Shared::new()),
        }),
    }
}

pin_project! {
    /// A timer that runs only while its task is idle. See [`idle_sleep`].
    pub struct IdleSleep {
        // Sleep timer, created only when finite idle timeout is requested.
        #[pin]
        timer: Option<Timer>,
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
        // Whether the task is busy and when it last became idle, shared with
        // the handles and guards that mark it busy.
        shared: Arc<Shared>,
    }
}

impl IdleSleep {
    /// A handle for marking the task busy, which can be cloned and sent to
    /// wherever its work is done.
    pub fn handle(&self) -> IdleHandle {
        IdleHandle {
            shared: self.timer.as_ref().map(|timer| timer.shared.clone()),
        }
    }

    /// Whether the task has anything to do right now, or `None` for a timer
    /// with no timeout, which does not count its task's work.
    pub fn is_busy(&self) -> Option<bool> {
        self.timer.as_ref().map(|timer| timer.shared.is_busy())
    }
}

/// Marks a task busy for as long as the guards it hands out are held. See
/// [`idle_sleep`].
#[derive(Clone, Debug)]
pub struct IdleHandle {
    // `None` when the timer has no timeout, so there is nothing to track.
    shared: Option<Arc<Shared>>,
}

impl IdleHandle {
    /// Counts the task as busy until the returned guard is dropped.
    pub fn guard(&self) -> IdleGuard {
        IdleGuard {
            shared: self.shared.as_ref().map(|shared| {
                shared.has_been_busy.store(true, Ordering::Relaxed);
                shared.busy.fetch_add(1, Ordering::Relaxed);
                shared.clone()
            }),
        }
    }

    #[cfg(test)]
    pub fn is_busy(&self) -> Option<bool> {
        self.shared.as_ref().map(|shared| shared.is_busy())
    }

    /// Approximately how long the task has been idle, or `None` if it is busy
    /// or its idleness is not tracked because the timer has no timeout.
    ///
    /// The operation is not atomic -- it may become busy right after it returns
    /// `Some`.
    pub fn idle_for(&self) -> Option<Duration> {
        self.shared
            .as_ref()
            .filter(|shared| !shared.is_busy())
            .map(|shared| shared.idle_for())
    }
}

/// Counts its task as busy for as long as it is held. See [`idle_sleep`].
#[must_use = "the task is only counted busy for as long as the guard is held"]
#[derive(Debug)]
pub struct IdleGuard {
    // `None` when the timer has no timeout, so there is nothing to count.
    shared: Option<Arc<Shared>>,
}

impl Drop for IdleGuard {
    fn drop(&mut self) {
        let Some(shared) = &self.shared else {
            return;
        };

        // Stored before the count falls, so a poll that sees the task idle
        // cannot read an `idle_since` from the previous idle period, find the
        // deadline already past, and close a connection that has just this
        // moment finished a long request.
        let idle_since = Instant::now() - shared.created_at;
        shared
            .idle_since_ms
            .store(idle_since.as_millis() as u64, Ordering::Relaxed);

        // Release, so a poll that reads the count this leaves also reads the
        // `idle_since` stored above.
        let previously_busy = shared.busy.fetch_sub(1, Ordering::Release);
        debug_assert!(
            previously_busy > 0,
            "guards are only ever created by IdleHandle::guard, which counts them"
        );
        if previously_busy != 1 {
            return;
        }

        // The last of the task's work is done, so a new idle period starts:
        // the timer may be reached again, and the task is woken to arm it from
        // the moment stored above.
        let mut state = shared.state.lock().unwrap();
        state.reached = false;
        if let Some(waker) = state.waker.take() {
            drop(state);
            waker.wake();
        }
    }
}

#[derive(Debug)]
struct Shared {
    /// Live [`IdleGuard`]s; the task is idle when this is zero.
    ///
    /// Outside the lock because every request takes and drops a guard, and
    /// [`IdleHandle::idle_for`] reads it. The lock is still taken whenever this
    /// reaches zero, so the timer is armed in step with it.
    busy: AtomicUsize,
    /// Whether a guard has ever been taken.
    has_been_busy: AtomicBool,
    /// When the timer was created, which `idle_since_ms` counts from.
    created_at: Instant,
    /// When the task last became idle, in milliseconds relative to
    /// `created_at`.
    idle_since_ms: AtomicU64,
    state: Mutex<State>,
}

impl Shared {
    fn new() -> Self {
        Self {
            busy: AtomicUsize::new(0),
            has_been_busy: AtomicBool::new(false),
            created_at: Instant::now(),
            idle_since_ms: AtomicU64::new(0),
            state: Mutex::default(),
        }
    }

    fn is_busy(&self) -> bool {
        // Acquire, so a caller that finds the task idle and then reads
        // `idle_since` reads the one stored by the guard that made it idle.
        self.busy.load(Ordering::Acquire) > 0
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
    /// Waker of the task polling the timer, woken when the task becomes idle.
    waker: Option<Waker>,
    /// Whether the timer has been reached since the task last became idle.
    reached: bool,
}

impl Future for IdleSleep {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Some(timer) = self.project().timer.as_pin_mut() else {
            // No deadline, so nothing ever makes this ready and nothing needs
            // to wake the task on its account.
            return Poll::Pending;
        };
        let mut timer = timer.project();
        let shared: &Shared = timer.shared;

        let mut state = shared.state.lock().unwrap();

        // Registering the waker under the same lock the last guard takes is
        // what stops a wake going missing: either the guard has yet to reach
        // the lock and will find the waker there, or it has already been
        // through, so this reads zero guards and `reached` cleared.
        if shared.is_busy() || state.reached {
            state.waker = Some(cx.waker().clone());
            return Poll::Pending;
        }

        // Armed from where it should be rather than from a flag saying it
        // needs arming: the deadline is a function of when the task last
        // became idle, so a timer that reports ready is one that is genuinely
        // due, whatever order the guard's store and this poll happened in.
        let deadline = shared.idle_since() + *timer.timeout;
        if timer.sleep.deadline() != deadline {
            timer.sleep.as_mut().reset(deadline);
        }

        // Still under the lock, so a guard dropped after the check above
        // clears `reached` after it is set here, not before.
        let poll = timer.sleep.poll(cx);
        state.reached = poll.is_ready();
        poll
    }
}
