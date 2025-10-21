// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{sync::Arc, time::SystemTime};

use iota_protocol_config::ProtocolConfig;
use starfish_config::{AuthorityIndex, Committee, Parameters};
#[cfg(test)]
use starfish_config::{NetworkKeyPair, ProtocolKeyPair};
#[cfg(test)]
use tempfile::TempDir;
use tokio::time::Instant;

#[cfg(test)]
use crate::metrics::test_metrics;
use crate::{block_header::BlockTimestampMs, metrics::Metrics};

/// Context contains per-epoch configuration and metrics shared by all
/// components of this authority.
#[derive(Clone)]
pub(crate) struct Context {
    /// Timestamp of the start of the current epoch.
    pub epoch_start_timestamp_ms: u64,
    /// Index of this authority in the committee.
    pub own_index: AuthorityIndex,
    /// Committee of the current epoch.
    pub committee: Committee,
    /// Parameters of this authority.
    pub parameters: Parameters,
    /// Protocol configuration of current epoch.
    pub protocol_config: ProtocolConfig,
    /// Metrics of this authority.
    pub metrics: Arc<Metrics>,
    /// Access to local clock
    pub clock: Arc<Clock>,
}

impl Context {
    pub(crate) fn new(
        epoch_start_timestamp_ms: u64,
        own_index: AuthorityIndex,
        committee: Committee,
        parameters: Parameters,
        protocol_config: ProtocolConfig,
        metrics: Arc<Metrics>,
        clock: Arc<Clock>,
    ) -> Self {
        Self {
            epoch_start_timestamp_ms,
            own_index,
            committee,
            parameters,
            protocol_config,
            metrics,
            clock,
        }
    }

    pub(crate) fn authority_hostname(&self, authority: AuthorityIndex) -> &str {
        self.committee.authority(authority).hostname.as_str()
    }

    /// Create a test context with a committee of given size and even stake
    #[cfg(test)]
    pub(crate) fn new_for_test(
        committee_size: usize,
    ) -> (Self, Vec<(NetworkKeyPair, ProtocolKeyPair)>) {
        let (committee, keypairs) =
            starfish_config::local_committee_and_keys(0, vec![1; committee_size]);
        let metrics = test_metrics();
        let temp_dir = TempDir::new().unwrap();
        let clock = Arc::new(Clock::default());

        let context = Context::new(
            0,
            AuthorityIndex::new_for_test(0),
            committee,
            Parameters {
                db_path: temp_dir.keep(),
                ..Default::default()
            },
            ProtocolConfig::get_for_max_version_UNSAFE(),
            metrics,
            clock,
        );
        (context, keypairs)
    }

    #[cfg(test)]
    pub(crate) fn with_epoch_start_timestamp_ms(mut self, epoch_start_timestamp_ms: u64) -> Self {
        self.epoch_start_timestamp_ms = epoch_start_timestamp_ms;
        self
    }

    #[cfg(test)]
    pub(crate) fn with_authority_index(mut self, authority: AuthorityIndex) -> Self {
        self.own_index = authority;
        self
    }

    #[cfg(test)]
    pub(crate) fn with_committee(mut self, committee: Committee) -> Self {
        self.committee = committee;
        self
    }

    #[cfg(test)]
    pub(crate) fn with_parameters(mut self, parameters: Parameters) -> Self {
        self.parameters = parameters;
        self
    }
}

/// A clock that allows to derive the current UNIX system timestamp while
/// guaranteeing that timestamp will be monotonically incremented, tolerating
/// ntp and system clock changes and corrections. Explicitly avoid to make
/// `[Clock]` cloneable to ensure that a single instance is shared behind an
/// `[Arc]` wherever is needed in order to make sure that consecutive calls to
/// receive the system timestamp will remain monotonically increasing.
pub struct Clock {
    initial_instant: Instant,
    initial_system_time: SystemTime,
    // `clock_drift` should be used only for testing
    #[cfg(any(test, msim))]
    clock_drift: BlockTimestampMs,
}

impl Default for Clock {
    fn default() -> Self {
        Self {
            initial_instant: Instant::now(),
            initial_system_time: SystemTime::now(),
            #[cfg(any(test, msim))]
            clock_drift: 0,
        }
    }
}

impl Clock {
    #[cfg(any(test, msim))]
    pub fn new_for_test(clock_drift: BlockTimestampMs) -> Self {
        Self {
            initial_instant: Instant::now(),
            initial_system_time: SystemTime::now(),
            clock_drift,
        }
    }

    // Returns the current time expressed as a UNIX timestamp in milliseconds.
    // Calculated with Tokio Instant to ensure monotonicity
    // and to allow testing with a tokio clock.
    pub(crate) fn timestamp_utc_ms(&self) -> BlockTimestampMs {
        let now: Instant = Instant::now();
        let monotonic_system_time = self
            .initial_system_time
            .checked_add(
                now.checked_duration_since(self.initial_instant)
                    .unwrap_or_else(|| {
                        panic!(
                            "current instant ({:?}) < initial instant ({:?})",
                            now, self.initial_instant
                        )
                    }),
            )
            .expect("Computing system time should not overflow");
        let timestamp = monotonic_system_time
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_else(|_| {
                panic!(
                    "system time ({:?}) < UNIX_EPOCH ({:?})",
                    monotonic_system_time,
                    SystemTime::UNIX_EPOCH,
                )
            })
            .as_millis() as BlockTimestampMs;
        // Apply clock drift only in test/msim environments to simulate clock skew
        // between nodes. In production builds, the clock_drift field doesn't exist,
        // and this returns the timestamp directly.
        #[cfg(any(test, msim))]
        {
            timestamp + self.clock_drift
        }
        #[cfg(not(any(test, msim)))]
        {
            timestamp
        }
    }
}
