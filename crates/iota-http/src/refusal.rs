// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Connections a listener refuses right after accept because it is at a
//! limit.

use std::time::{Duration, Instant};

/// The limit a refused connection met. The names are those of the `Config`
/// settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, strum::IntoStaticStr, strum::EnumIter)]
#[strum(serialize_all = "snake_case")]
pub(crate) enum RefusalReason {
    MaxConnections,
    MaxPendingConnections,
}

/// Decides how a refusal is logged: the first refusal in a minute at `warn`,
/// the others at `debug`.
#[derive(Debug, Default)]
pub(crate) struct RefusalLog {
    last_warning: Option<Instant>,
    /// Refusals logged at `debug` since the last warning.
    suppressed: u64,
}

impl RefusalLog {
    const WARNING_INTERVAL: Duration = Duration::from_secs(60);

    /// Records a refusal at `now`. Returns `Some` with the number of refusals
    /// logged at `debug` since the last warning when this refusal is to be
    /// logged at `warn`, and `None` when it is to be logged at `debug`.
    pub(crate) fn record(&mut self, now: Instant) -> Option<u64> {
        let warned_recently = self
            .last_warning
            .is_some_and(|last| now.saturating_duration_since(last) < Self::WARNING_INTERVAL);
        if warned_recently {
            self.suppressed += 1;
            return None;
        }
        self.last_warning = Some(now);
        Some(std::mem::take(&mut self.suppressed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_refusal_in_a_minute_is_logged_at_warn() {
        let mut log = RefusalLog::default();
        let start = Instant::now();

        assert_eq!(log.record(start), Some(0), "the first refusal is a warning");
        assert_eq!(
            log.record(start + Duration::from_secs(59)),
            None,
            "a refusal in the same minute is logged at debug"
        );
        assert_eq!(
            log.record(start + Duration::from_secs(60)),
            Some(1),
            "a minute later the next refusal is a warning that counts the one logged at debug"
        );
        assert_eq!(log.record(start + Duration::from_secs(61)), None);
    }
}
