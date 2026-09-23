// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

pub mod address_metrics_processor;
pub mod move_call_metrics_processor;
pub mod network_metrics_processor;
pub mod processor_orchestrator;

use tracing::info;

/// Determines the cursor position for a processor to resume from.
///
/// # Behavior
/// - **Stored Cursor:** Used if it points to valid data currently in the
///   database.
/// - **Fallback (`min_available`):** If the stored cursor points to pruned data
///   (or data missing after a snapshot restore), it falls back to the key
///   immediately below `min_available`.
///
/// Processing begins at `cursor + 1`, meaning the first key handled will be
/// `min_available` itself.
///
/// Logs are emitted when the cursor shifts, as metrics remain absent prior to
/// this event.
pub(crate) fn resume_cursor(stored: i64, min_available: i64) -> i64 {
    let resumed = stored.max(min_available - 1);
    if resumed != stored {
        info!(
            "cursor {stored} is below the first available key {min_available}, resuming from there"
        );
    }
    resumed
}

#[cfg(test)]
mod tests {
    use super::resume_cursor;

    #[test]
    fn stored_cursor_is_kept_when_nothing_is_missing() {
        assert_eq!(resume_cursor(0, 0), 0);
        assert_eq!(resume_cursor(500, 0), 500);
        assert_eq!(resume_cursor(500, 200), 500);
    }

    #[test]
    fn cursor_moves_to_the_key_below_the_lower_bound() {
        // Resuming at 999 makes the first processed key 1000, the first one
        // that exists.
        assert_eq!(resume_cursor(0, 1000), 999);
        assert_eq!(resume_cursor(500, 1000), 999);
    }

    #[test]
    fn cursor_right_below_the_lower_bound_is_unchanged() {
        assert_eq!(resume_cursor(999, 1000), 999);
    }
}
