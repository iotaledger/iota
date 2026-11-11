// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use crate::error::{ConsensusError, ConsensusResult};

/// Runs the closure on the blocking pool like `tokio::task::spawn_blocking`,
/// but resolves to `ConsensusError::Shutdown` instead of a `JoinError` when
/// the task is cancelled by runtime shutdown. A panic in the closure is
/// resumed on the calling thread.
pub(crate) async fn spawn_blocking<F, T>(f: F) -> ConsensusResult<T>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(spawn_blocking_join_error)
}

/// A blocking task's join error is either a panic, resumed here, or a
/// cancellation, which only happens when the runtime is shutting down.
fn spawn_blocking_join_error(e: tokio::task::JoinError) -> ConsensusError {
    if e.is_panic() {
        std::panic::resume_unwind(e.into_panic());
    }
    ConsensusError::Shutdown
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A blocking task racing with runtime shutdown is cancelled instead of
    /// run, so its join handle resolves to a cancellation error.
    #[tokio::test]
    async fn cancelled_blocking_task_maps_to_shutdown() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let handle = runtime.handle().clone();
        runtime.shutdown_background();

        let join_error = handle.spawn_blocking(|| ()).await.unwrap_err();
        assert!(join_error.is_cancelled());
        assert!(matches!(
            spawn_blocking_join_error(join_error),
            ConsensusError::Shutdown
        ));
    }

    #[tokio::test]
    #[should_panic(expected = "boom")]
    async fn panic_in_blocking_task_is_resumed() {
        let _ = spawn_blocking(|| panic!("boom")).await;
    }
}
