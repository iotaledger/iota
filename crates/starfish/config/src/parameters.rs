// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{path::PathBuf, time::Duration};

use serde::{Deserialize, Serialize};

/// Operational configurations of a consensus authority.
///
/// All fields should tolerate inconsistencies among authorities, without
/// affecting safety of the protocol. Otherwise, they need to be part of IOTA
/// protocol config or epoch state on-chain.
///
/// NOTE: fields with default values are specified in the serde default
/// functions. Most operators should not need to specify any field, except
/// db_path.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Parameters {
    /// Path to consensus DB for this epoch. Required when initializing
    /// consensus. This is calculated based on user configuration for base
    /// directory.
    #[serde(skip)]
    pub db_path: PathBuf,

    /// Time to wait for parent round leader before sealing a block, from when
    /// parent round has a quorum.
    #[serde(default = "Parameters::default_leader_timeout")]
    pub leader_timeout: Duration,

    /// Sustained spacing between own blocks: long-run production never exceeds
    /// one block per `min_block_delay`. This avoids generating too many rounds
    /// when latency is low. This is especially necessary for tests running
    /// locally. If setting a non-default value, it should be set low enough
    /// to avoid reducing round rate and increasing latency in realistic and
    /// distributed configurations.
    #[serde(default = "Parameters::default_min_block_delay")]
    pub min_block_delay: Duration,

    /// Soft counterpart of `leader_timeout`: after this duration we are
    /// willing to propose a block even without a strong-vote quorum, to avoid
    /// liveness stalls when leader data is slow to propagate. Fires earlier
    /// than `leader_timeout` and does not force block creation on its own.
    #[serde(default = "Parameters::default_soft_leader_timeout")]
    pub soft_leader_timeout: Duration,

    /// Window bounding own block production together with `min_block_delay`:
    /// idle time accrues budget for bursts of up to `block_rate_window /
    /// min_block_delay` back-to-back blocks, letting a validator that fell
    /// behind catch up on rounds instead of skipping them. Set at or below
    /// `min_block_delay` to disable bursting (fixed spacing between blocks).
    #[serde(default = "Parameters::default_block_rate_window")]
    pub block_rate_window: Duration,

    /// Number of block headers to fetch per commit sync request.
    #[serde(default = "Parameters::default_max_headers_per_commit_sync_fetch")]
    pub max_headers_per_commit_sync_fetch: usize,

    /// Number of transactions to fetch per commit sync request.
    #[serde(default = "Parameters::default_max_transactions_per_commit_sync_fetch")]
    pub max_transactions_per_commit_sync_fetch: usize,

    /// Number of block headers to fetch per header sync (periodic or live)
    /// request.
    #[serde(default = "Parameters::default_max_headers_per_header_sync_fetch")]
    pub max_headers_per_header_sync_fetch: usize,

    /// Number of transactions to fetch per transaction sync request.
    #[serde(default = "Parameters::default_max_transactions_per_transaction_sync_fetch")]
    pub max_transactions_per_transaction_sync_fetch: usize,

    /// Time to wait during node start up until the node has synced the last
    /// proposed block via the network peers. When set to `0` the sync
    /// mechanism is disabled. This property is meant to be used for amnesia
    /// recovery.
    #[serde(default = "Parameters::default_sync_last_known_own_block_timeout")]
    pub sync_last_known_own_block_timeout: Duration,

    /// The number of rounds of blocks to be kept in the Dag state cache per
    /// authority. The larger the number the more the blocks that will be
    /// kept in memory allowing minimising any potential disk access.
    /// Value should be at minimum 50 rounds to ensure node performance, but
    /// being too large can be expensive in memory usage.
    #[serde(default = "Parameters::default_dag_state_cached_rounds")]
    pub dag_state_cached_rounds: u32,

    /// Rounds a header from a far-future-bounded source may lead the locally
    /// accepted frontier, in addition to `dag_state_cached_rounds`, before it
    /// is dropped as too far ahead to connect.
    #[serde(default = "Parameters::default_peer_round_ahead_margin")]
    pub peer_round_ahead_margin: u32,

    // Number of authorities commit syncer fetches in parallel.
    // Both commits in a range and blocks referenced by the commits are fetched per authority.
    #[serde(default = "Parameters::default_commit_sync_parallel_fetches")]
    pub commit_sync_parallel_fetches: usize,

    // Number of commits to fetch in a batch, also the maximum number of commits returned per
    // fetch. If this value is set too small, fetching becomes inefficient.
    // If this value is set too large, it can result in load imbalance and stragglers.
    #[serde(default = "Parameters::default_commit_sync_batch_size")]
    pub commit_sync_batch_size: u32,

    // This affects the maximum number of commit batches being fetched, and those fetched but not
    // processed as consensus output, before throttling of outgoing commit fetches starts.
    #[serde(default = "Parameters::default_commit_sync_batches_ahead")]
    pub commit_sync_batches_ahead: usize,

    /// Maximum number of commits scanned and replayed per batch during
    /// recovery, bounding peak memory when a large unprocessed range is
    /// replayed at startup.
    #[serde(default = "Parameters::default_commit_recovery_batch_size")]
    pub commit_recovery_batch_size: u32,

    /// Maximum number of headers to be included in a bundle. Headers exceeding
    /// the max allowed limit will be truncated.
    #[serde(default = "Parameters::default_max_headers_per_bundle")]
    pub max_headers_per_bundle: usize,

    /// Maximum number of transaction shards to be included in a bundle. Shards
    /// exceeding the max allowed limit will be truncated.
    #[serde(default = "Parameters::default_max_shards_per_bundle")]
    pub max_shards_per_bundle: usize,

    /// Tonic network settings.
    #[serde(default = "TonicParameters::default")]
    pub tonic: TonicParameters,

    // Number of commits to fetch in a batch for fast commit syncer, also the maximum number of
    // commits returned per fetch. If this value is set too small, fetching becomes
    // inefficient. If this value is set too large, it can result in load imbalance and
    // stragglers.
    #[serde(default = "Parameters::default_fast_commit_sync_batch_size")]
    pub fast_commit_sync_batch_size: u32,

    // Gap threshold for switching between commit syncers. When the gap between quorum and local
    // commit index is larger than this threshold, FastCommitSyncer fetches. Otherwise,
    // CommitSyncer fetches.
    #[serde(default = "Parameters::default_commit_sync_gap_threshold")]
    pub commit_sync_gap_threshold: u32,

    /// Enable FastCommitSyncer for faster recovery from large commit gaps.
    /// Enabled by default; operators can disable it locally if bugs are
    /// discovered.
    #[serde(default = "Parameters::default_enable_fast_commit_syncer")]
    pub enable_fast_commit_syncer: bool,

    /// Enable adaptive acknowledgment filtering for StarfishSpeed.
    /// Local heuristic that drops acks for authorities persistently blamed
    /// by recent strong-vote masks. Effective only when the protocol-level
    /// `consensus_starfish_speed` flag is also on. Enabled by default;
    /// operators can disable it locally without a protocol change.
    #[serde(default = "Parameters::default_enable_starfish_speed_adaptive_acknowledgments")]
    pub enable_starfish_speed_adaptive_acknowledgments: bool,

    /// Port for the DAG visualizer gRPC server (localhost only).
    /// When set, starts a debugging server for real-time DAG visualization.
    /// Only has an effect when the `dag-visualizer` feature is compiled in.
    /// Disabled by default (None).
    #[serde(default)]
    pub dag_visualizer_port: Option<u16>,
}

impl Parameters {
    /// Threshold for the number of commits sent to the consumer but not yet
    /// handled, above which commit producers (commit syncers, commit observer
    /// recovery) pause to let the consumer catch up.
    pub fn unhandled_commits_threshold(&self) -> u32 {
        self.commit_sync_batch_size * (self.commit_sync_batches_ahead as u32)
    }

    pub(crate) fn default_leader_timeout() -> Duration {
        Duration::from_millis(200)
    }

    pub(crate) fn default_min_block_delay() -> Duration {
        if cfg!(msim) || std::env::var("__TEST_ONLY_CONSENSUS_USE_LONG_MIN_BLOCK_DELAY").is_ok() {
            // Checkpoint building and execution cannot keep up with high commit rate in
            // simtests, leading to long reconfiguration delays. This is because
            // simtest is single threaded, and spending too much time in
            // consensus can lead to starvation elsewhere.
            Duration::from_millis(400)
        } else if cfg!(test) {
            // Avoid excessive CPU, data and logs in tests.
            Duration::from_millis(250)
        } else {
            // For production, use min delay between block being set to 50ms, reducing the
            // block rate to 20 blocks/sec
            Duration::from_millis(50)
        }
    }

    pub(crate) fn default_soft_leader_timeout() -> Duration {
        Duration::from_millis(5)
    }

    pub(crate) fn default_block_rate_window() -> Duration {
        Duration::from_secs(2)
    }

    /// Burst capacity: maximum number of own blocks within `block_rate_window`
    /// (40 in production, 5 in msim, 8 in tests with the default window).
    pub fn block_rate_burst(&self) -> u64 {
        let interval_ms = self.min_block_delay.as_millis().max(1) as u64;
        (self.block_rate_window.as_millis() as u64 / interval_ms).max(1)
    }

    /// Highest round a header from a far-future-bounded source may have,
    /// relative to the accepted `frontier`, to still be close enough to
    /// connect; headers above this are too far ahead and dropped.
    pub fn far_future_round_ceiling(&self, frontier: u32) -> u32 {
        frontier
            .saturating_add(self.dag_state_cached_rounds)
            .saturating_add(self.peer_round_ahead_margin)
    }

    /// Maximum number of block headers served per fetch request, depending on
    /// whether the request comes from commit sync or the header synchronizer.
    pub fn max_headers_per_fetch(&self, commit_sync: bool) -> usize {
        if commit_sync {
            self.max_headers_per_commit_sync_fetch
        } else {
            self.max_headers_per_header_sync_fetch
        }
    }

    /// Validates local consensus parameters, rejecting zero values that can
    /// lead to synchronization problems. Returns a description of the first
    /// offending field.
    pub fn validate(&self) -> Result<(), String> {
        let positive_fields = [
            (
                "max_headers_per_commit_sync_fetch",
                self.max_headers_per_commit_sync_fetch as u128,
            ),
            (
                "max_transactions_per_commit_sync_fetch",
                self.max_transactions_per_commit_sync_fetch as u128,
            ),
            (
                "max_headers_per_header_sync_fetch",
                self.max_headers_per_header_sync_fetch as u128,
            ),
            (
                "max_transactions_per_transaction_sync_fetch",
                self.max_transactions_per_transaction_sync_fetch as u128,
            ),
            (
                "dag_state_cached_rounds",
                self.dag_state_cached_rounds as u128,
            ),
            (
                "commit_sync_parallel_fetches",
                self.commit_sync_parallel_fetches as u128,
            ),
            (
                "commit_sync_batch_size",
                self.commit_sync_batch_size as u128,
            ),
            (
                "commit_recovery_batch_size",
                self.commit_recovery_batch_size as u128,
            ),
            (
                "commit_sync_batches_ahead",
                self.commit_sync_batches_ahead as u128,
            ),
            (
                "max_headers_per_bundle",
                self.max_headers_per_bundle as u128,
            ),
            ("max_shards_per_bundle", self.max_shards_per_bundle as u128),
            (
                "fast_commit_sync_batch_size",
                self.fast_commit_sync_batch_size as u128,
            ),
            (
                "tonic.connection_buffer_size",
                self.tonic.connection_buffer_size as u128,
            ),
            (
                "tonic.excessive_message_size",
                self.tonic.excessive_message_size as u128,
            ),
            (
                "tonic.message_size_limit",
                self.tonic.message_size_limit as u128,
            ),
            (
                "tonic.keepalive_interval",
                self.tonic.keepalive_interval.as_nanos(),
            ),
        ];
        for (name, value) in positive_fields {
            if value == 0 {
                return Err(format!("{name} must be positive"));
            }
        }
        Ok(())
    }

    // Maximum number of block headers to fetch per commit sync request.
    pub(crate) fn default_max_headers_per_commit_sync_fetch() -> usize {
        if cfg!(msim) {
            // Exercise hitting blocks per fetch limit.
            10
        } else {
            1000
        }
    }

    // Maximum number of transactions to fetch per commit sync request.
    pub(crate) fn default_max_transactions_per_commit_sync_fetch() -> usize {
        if cfg!(msim) {
            // Exercise hitting transactions per fetch limit.
            10
        } else {
            1000
        }
    }

    // Maximum number of block headers to fetch per header sync (periodic or
    // live) request.
    pub(crate) fn default_max_headers_per_header_sync_fetch() -> usize {
        if cfg!(msim) {
            // Exercise hitting blocks per fetch limit.
            10
        } else {
            // TODO: This might should match the value of block headers in the bundle.
            100
        }
    }

    // Maximum number of transactions to fetch per transaction sync request.
    pub(crate) fn default_max_transactions_per_transaction_sync_fetch() -> usize {
        if cfg!(msim) { 10 } else { 1000 }
    }

    pub(crate) fn default_sync_last_known_own_block_timeout() -> Duration {
        if cfg!(msim) {
            Duration::from_millis(500)
        } else {
            // Here we prioritise liveness over the complete de-risking of block
            // equivocation. 5 seconds in the majority of cases should be good
            // enough for this given a healthy network.
            Duration::from_secs(5)
        }
    }

    pub(crate) fn default_dag_state_cached_rounds() -> u32 {
        if cfg!(msim) {
            // Exercise reading blocks from store.
            5
        } else {
            500
        }
    }

    pub(crate) fn default_peer_round_ahead_margin() -> u32 {
        1000
    }

    pub(crate) fn default_commit_sync_parallel_fetches() -> usize {
        8
    }

    pub(crate) fn default_commit_sync_batch_size() -> u32 {
        if cfg!(msim) {
            // Exercise commit sync.
            5
        } else {
            100
        }
    }

    pub(crate) fn default_commit_recovery_batch_size() -> u32 {
        if cfg!(msim) { 3 } else { 250 }
    }

    pub(crate) fn default_commit_sync_batches_ahead() -> usize {
        // This is set to be a multiple of default commit_sync_parallel_fetches to allow
        // fetching ahead, while keeping the total number of inflight fetches
        // and unprocessed fetched commits limited.
        32
    }

    pub(crate) fn default_max_headers_per_bundle() -> usize {
        150
    }

    pub(crate) fn default_max_shards_per_bundle() -> usize {
        150
    }

    pub(crate) fn default_fast_commit_sync_batch_size() -> u32 {
        if cfg!(msim) {
            // Exercise fast commit sync.
            5
        } else {
            // With ~10KB per commit and 4MB max message size, 1000 commits (~10MB) requires
            // chunking. The server will chunk commits across multiple response messages.
            1000
        }
    }

    pub(crate) fn default_commit_sync_gap_threshold() -> u32 {
        if cfg!(msim) {
            // Use smaller threshold for testing.
            10
        } else {
            // When gap > 1000, FastCommitSyncer is more efficient.
            // When gap <= 1000, CommitSyncer handles incremental sync.
            1000
        }
    }

    pub(crate) fn default_enable_fast_commit_syncer() -> bool {
        // Enabled by default. Operators can disable it locally if bugs are discovered,
        // without waiting for a protocol upgrade.
        true
    }

    pub(crate) fn default_enable_starfish_speed_adaptive_acknowledgments() -> bool {
        true
    }
}

impl Default for Parameters {
    fn default() -> Self {
        Self {
            db_path: PathBuf::default(),
            leader_timeout: Parameters::default_leader_timeout(),
            min_block_delay: Parameters::default_min_block_delay(),
            soft_leader_timeout: Parameters::default_soft_leader_timeout(),
            block_rate_window: Parameters::default_block_rate_window(),
            max_headers_per_commit_sync_fetch:
                Parameters::default_max_headers_per_commit_sync_fetch(),
            max_transactions_per_commit_sync_fetch:
                Parameters::default_max_transactions_per_commit_sync_fetch(),
            max_headers_per_header_sync_fetch:
                Parameters::default_max_headers_per_header_sync_fetch(),
            max_transactions_per_transaction_sync_fetch:
                Parameters::default_max_transactions_per_transaction_sync_fetch(),
            sync_last_known_own_block_timeout:
                Parameters::default_sync_last_known_own_block_timeout(),
            dag_state_cached_rounds: Parameters::default_dag_state_cached_rounds(),
            peer_round_ahead_margin: Parameters::default_peer_round_ahead_margin(),
            commit_sync_parallel_fetches: Parameters::default_commit_sync_parallel_fetches(),
            commit_sync_batch_size: Parameters::default_commit_sync_batch_size(),
            commit_sync_batches_ahead: Parameters::default_commit_sync_batches_ahead(),
            commit_recovery_batch_size: Parameters::default_commit_recovery_batch_size(),
            max_headers_per_bundle: Parameters::default_max_headers_per_bundle(),
            max_shards_per_bundle: Parameters::default_max_shards_per_bundle(),
            tonic: TonicParameters::default(),
            fast_commit_sync_batch_size: Parameters::default_fast_commit_sync_batch_size(),
            commit_sync_gap_threshold: Parameters::default_commit_sync_gap_threshold(),
            enable_fast_commit_syncer: Parameters::default_enable_fast_commit_syncer(),
            enable_starfish_speed_adaptive_acknowledgments:
                Parameters::default_enable_starfish_speed_adaptive_acknowledgments(),
            dag_visualizer_port: None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TonicParameters {
    /// Keepalive interval and timeouts for both client and server.
    ///
    /// If unspecified, this will default to 5s.
    #[serde(default = "TonicParameters::default_keepalive_interval")]
    pub keepalive_interval: Duration,

    /// Size of various per-connection buffers.
    ///
    /// If unspecified, this will default to 32MiB.
    #[serde(default = "TonicParameters::default_connection_buffer_size")]
    pub connection_buffer_size: usize,

    /// Messages over this size threshold will increment a counter.
    ///
    /// If unspecified, this will default to 16MiB.
    #[serde(default = "TonicParameters::default_excessive_message_size")]
    pub excessive_message_size: usize,

    /// Hard message size limit for both requests and responses.
    /// This value is higher than strictly necessary, to allow overheads.
    /// Message size targets and soft limits are computed based on this value.
    ///
    /// If unspecified, this will default to 1GiB.
    #[serde(default = "TonicParameters::default_message_size_limit")]
    pub message_size_limit: usize,

    /// Maximum number of concurrent HTTP/2 streams a peer may open on a single
    /// connection. Bounds per-connection request fan-out.
    ///
    /// `0` (the default) disables the limit, leaving the transport default.
    #[serde(default)]
    pub max_concurrent_streams: u32,

    /// Server-side fallback deadline for requests that omit a `grpc-timeout`
    /// header. The long-lived block-subscription stream is always exempt.
    ///
    /// A zero duration (the default) disables the fallback deadline.
    #[serde(default)]
    pub request_timeout: Duration,

    /// Hard size limit for inbound (decoded) requests. Consensus requests are
    /// small (ref lists); large payloads belong to responses, bounded by
    /// `message_size_limit`. A smaller inbound bound shrinks the memory a
    /// single in-flight request can pin before its handler runs.
    ///
    /// `0` (the default) falls back to `message_size_limit`.
    #[serde(default)]
    pub max_inbound_message_size: usize,

    /// Per-peer, per-RPC admission caps for the inbound consensus server.
    #[serde(default)]
    pub admission: AdmissionParameters,
}

impl TonicParameters {
    fn default_keepalive_interval() -> Duration {
        Duration::from_secs(5)
    }

    fn default_connection_buffer_size() -> usize {
        32 << 20
    }

    fn default_excessive_message_size() -> usize {
        16 << 20
    }

    fn default_message_size_limit() -> usize {
        64 << 20
    }

    /// Fills the inbound resource bounds that are still at their inert
    /// defaults with the protective preset (sized for ~100-validator
    /// committees). Bounds an operator configured explicitly are kept, as are
    /// the transport settings the preset does not cover (keepalive, buffers,
    /// `message_size_limit`). A bound explicitly configured to its inert value
    /// still receives the preset; running without a bound requires disabling
    /// the preset itself (`CONSENSUS_GRPC_PROTECTIVE_LIMITS=0`).
    pub fn apply_protective(&mut self) {
        if self.max_concurrent_streams == 0 {
            self.max_concurrent_streams = 64;
        }
        if self.request_timeout.is_zero() {
            self.request_timeout = Duration::from_secs(120);
        }
        if self.max_inbound_message_size == 0 {
            self.max_inbound_message_size = 1 << 20;
        }
        if self.admission.is_inert() {
            self.admission = AdmissionParameters::protective();
        }
    }

    /// The inert defaults with the protective bounds applied.
    pub fn protective() -> Self {
        let mut params = Self::default();
        params.apply_protective();
        params
    }
}

impl Default for TonicParameters {
    fn default() -> Self {
        Self {
            keepalive_interval: TonicParameters::default_keepalive_interval(),
            connection_buffer_size: TonicParameters::default_connection_buffer_size(),
            excessive_message_size: TonicParameters::default_excessive_message_size(),
            message_size_limit: TonicParameters::default_message_size_limit(),
            max_concurrent_streams: 0,
            request_timeout: Duration::ZERO,
            max_inbound_message_size: 0,
            admission: AdmissionParameters::default(),
        }
    }
}

/// Per-peer, per-RPC concurrency caps for the inbound consensus gRPC server.
///
/// Each cap bounds how many concurrent requests of one RPC group a single
/// committee peer (keyed on its authenticated authority index) may have in
/// flight; a peer cannot consume another peer's budget. These are local,
/// non-protocol parameters — heterogeneous values across authorities are safe,
/// so they can be rolled out and tuned per node.
///
/// `0` (the default for every cap) disables admission for that group. At node
/// start the protective preset fills the caps when all of them are left at
/// `0`; a caps block with any explicitly configured value is kept as-is, and
/// `CONSENSUS_GRPC_PROTECTIVE_LIMITS=0` disables the preset entirely. Preset
/// values, sized for ~100-validator committees and the local synchronizer
/// fan-out toward one server: subscriptions 2, header fetches 32, transaction
/// fetches 16, commit fetches `commit_sync_parallel_fetches` (8).
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct AdmissionParameters {
    /// Max concurrent block-subscription streams per peer.
    #[serde(default)]
    pub max_subscriptions_per_peer: u32,

    /// Max concurrent header fetches per peer
    /// (`fetch_block_headers` + `fetch_latest_block_headers`).
    #[serde(default)]
    pub max_header_fetches_per_peer: u32,

    /// Max concurrent transaction fetches per peer (`fetch_transactions`).
    #[serde(default)]
    pub max_transaction_fetches_per_peer: u32,

    /// Max concurrent commit fetches per peer
    /// (`fetch_commits` + `fetch_commits_and_transactions`).
    #[serde(default)]
    pub max_commit_fetches_per_peer: u32,
}

impl AdmissionParameters {
    /// Preset sized for ~100-validator committees and the local synchronizer
    /// fan-out toward one server.
    pub fn protective() -> Self {
        Self {
            max_subscriptions_per_peer: 2,
            max_header_fetches_per_peer: 32,
            max_transaction_fetches_per_peer: 16,
            max_commit_fetches_per_peer: Parameters::default_commit_sync_parallel_fetches() as u32,
        }
    }

    /// True when every cap is `0`, i.e. admission control is disabled.
    pub fn is_inert(&self) -> bool {
        self.max_subscriptions_per_peer == 0
            && self.max_header_fetches_per_peer == 0
            && self.max_transaction_fetches_per_peer == 0
            && self.max_commit_fetches_per_peer == 0
    }
}
