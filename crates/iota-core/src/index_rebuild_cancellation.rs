// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Marks the errors an index rebuild produces when a shutdown cancels it, so
//! callers can tell them apart from a real failure that happens to race the
//! shutdown.

use std::fmt;

use iota_types::storage::error::Error as StorageError;

/// Error source marking index work abandoned because the node is shutting
/// down.
#[derive(Debug)]
pub struct RebuildCancelled(String);

impl RebuildCancelled {
    /// A [`StorageError`] that reads as `message` and that
    /// [`is_cancelled`] recognizes.
    pub(crate) fn error(message: impl Into<String>) -> StorageError {
        StorageError::custom(Self(message.into()))
    }
}

impl fmt::Display for RebuildCancelled {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for RebuildCancelled {}

/// Reports whether `error` comes from a rebuild cancelled by a shutdown,
/// including one rewrapped with a caller's own message.
pub fn is_cancelled(error: &StorageError) -> bool {
    let mut error: Option<&(dyn std::error::Error + 'static)> = Some(error);
    while let Some(current) = error {
        if current.downcast_ref::<RebuildCancelled>().is_some() {
            return true;
        }
        error = current.source();
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A failure racing a shutdown must stay a failure: the caller decides by
    /// the error, not by the cancellation flag, which is set either way.
    #[test]
    fn a_plain_failure_is_not_reported_as_cancelled() {
        assert!(!is_cancelled(&StorageError::missing(
            "missing checkpoint 3"
        )));
        assert!(!is_cancelled(&StorageError::custom(
            "the live object set scan was cancelled"
        )));
    }

    /// The rebuild rewraps the cancellation in its own message, which must not
    /// hide the marker from the node's exit path.
    #[test]
    fn the_marker_survives_rewrapping() {
        let cancelled = RebuildCancelled::error("the live object set scan was cancelled");
        let rewrapped = RebuildCancelled::error(format!(
            "the JSON-RPC index rebuild was cancelled by shutdown: {cancelled}"
        ));

        assert!(is_cancelled(&cancelled));
        assert!(is_cancelled(&rewrapped));
        assert!(rewrapped.to_string().contains("cancelled by shutdown"));
    }
}
