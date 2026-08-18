// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Protocol level traffic control. Tallies are charged to the spam and error
//! policies inline, so a breaching client is blocked locally, or queued for the
//! firewall, before [`TrafficController::tally`] returns.

pub mod metrics;
pub mod nodefw_client;
pub mod nodefw_test_server;
pub mod policies;

use std::{
    collections::HashSet,
    fmt::Debug,
    fs,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    ops::Add,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant, SystemTime},
};

use dashmap::DashMap;
use fs::File;
use iota_common::fatal;
use iota_metrics::spawn_monitored_task;
use iota_types::{
    error::IotaError,
    traffic_control::{
        ClientIdSource, PolicyConfig, PolicyType, RemoteFirewallConfig,
        TrafficControlReconfigParams, Weight,
    },
};
use parking_lot::Mutex;
use prometheus_filtered::IntGauge;
use rand::Rng;
use tokio::{
    sync::{mpsc, mpsc::error::TrySendError},
    time,
};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use self::metrics::TrafficControllerMetrics;
use crate::{
    nodefw_client::{BlockAddress, BlockAddresses, NodeFWClient},
    policies::{MAX_CLIENT_THRESHOLD, PolicyResponse, TrafficControlPolicy, TrafficTally},
};

const CLEAR_BLOCKLIST_INTERVAL: Duration = Duration::from_secs(3);
const DEADMANS_SWITCH_POLL_INTERVAL: Duration = Duration::from_secs(1);
/// Number of pending firewall delegations held before further ones are applied
/// locally instead.
const FIREWALL_DELEGATION_QUEUE_SIZE: usize = 256;

type Blocklist = Arc<DashMap<IpAddr, SystemTime>>;

#[derive(Clone)]
struct Blocklists {
    clients: Blocklist,
    proxied_clients: Blocklist,
}

#[derive(Clone)]
enum Acl {
    Tally(Arc<TallyState>),
    /// If this variant is set, then we do no tallying or running
    /// of background tasks, and instead simply block all IPs not
    /// in the allowlist on calls to `check`. The allowlist should
    /// only be populated once at initialization.
    Allowlist(Vec<IpAddr>),
}

/// Spam and error policies, along with the state shared by the paths that
/// charge them. Absent in allowlist mode, which does no tallying.
struct TallyState {
    spam_policy: Arc<TrafficControlPolicy>,
    error_policy: Arc<TrafficControlPolicy>,
    blocklists: Blocklists,
    firewall_delegation: Option<FirewallDelegation>,
    /// Whether the firewall drain file is present, refreshed by the dead man's
    /// switch. Delegation pauses while it is.
    drainfile_present: Arc<AtomicBool>,
    shutdown: CancellationToken,
}

/// Queue of blocks handed to the remote firewall, so that the request thread
/// never waits on the delegation request.
struct FirewallDelegation {
    sender: mpsc::Sender<Vec<DelegatedBlock>>,
    /// Clients whose block is queued or in flight, so that a client breaching
    /// on every request enqueues at most one block per firewall roundtrip.
    pending: Arc<Mutex<HashSet<IpAddr>>>,
    destination_port: u16,
    delegate_spam_blocking: bool,
    delegate_error_blocking: bool,
}

/// A block queued for the remote firewall, tagged with whether it targets the
/// proxied client so that a failed delegation lands in the right local
/// blocklist.
struct DelegatedBlock {
    client: IpAddr,
    address: BlockAddress,
    proxied: bool,
}

impl Drop for TallyState {
    fn drop(&mut self) {
        self.shutdown.cancel();
    }
}

#[derive(Clone)]
pub struct TrafficController {
    acl: Acl,
    policy_config: Arc<PolicyConfig>,
    metrics: Arc<TrafficControllerMetrics>,
    // Read on the request path in `check` and toggled by the admin API.
    dry_run: Arc<AtomicBool>,
}

impl Debug for TrafficController {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // NOTE: we do not want to print the contents of the blocklists to logs
        // given that (1) it contains all requests IPs, and (2) it could be quite
        // large. Instead, we print lengths of the blocklists. Further, we prefer
        // to get length from the metrics rather than from the blocklists themselves
        // to avoid unnecessarily acquiring the read lock.
        f.debug_struct("TrafficController")
            .field(
                "connection_ip_blocklist_len",
                &self.metrics.connection_ip_blocklist_len.get(),
            )
            .field(
                "proxy_ip_blocklist_len",
                &self.metrics.proxy_ip_blocklist_len.get(),
            )
            .finish()
    }
}

impl TrafficController {
    pub fn init(
        policy_config: PolicyConfig,
        metrics: Arc<TrafficControllerMetrics>,
        fw_config: Option<RemoteFirewallConfig>,
    ) -> Self {
        metrics.dry_run_enabled.set(policy_config.dry_run as i64);
        let dry_run = Arc::new(AtomicBool::new(policy_config.dry_run));

        let acl = match &policy_config.allow_list {
            Some(allow_list) => Acl::Allowlist(parse_allowlist(allow_list)),
            None => {
                let state = spawn_tally_state(&policy_config, &metrics, fw_config.as_ref());
                set_policy_config_metrics(&state, &policy_config, &metrics);
                Acl::Tally(Arc::new(state))
            }
        };
        Self {
            acl,
            policy_config: Arc::new(policy_config),
            metrics,
            dry_run,
        }
    }

    pub fn init_for_test(
        policy_config: PolicyConfig,
        fw_config: Option<RemoteFirewallConfig>,
    ) -> Self {
        Self::init(
            policy_config,
            Arc::new(TrafficControllerMetrics::new_for_tests()),
            fw_config,
        )
    }

    fn tally_state(&self) -> Option<&TallyState> {
        match &self.acl {
            Acl::Tally(state) => Some(state),
            Acl::Allowlist(_) => None,
        }
    }

    pub fn get_current_state(&self) -> TrafficControlReconfigParams {
        TrafficControlReconfigParams {
            error_threshold: self
                .tally_state()
                .and_then(|state| state.error_policy.client_threshold()),
            spam_threshold: self
                .tally_state()
                .and_then(|state| state.spam_policy.client_threshold()),
            dry_run: Some(self.dry_run.load(Ordering::Relaxed)),
        }
    }

    /// Applies an operator initiated policy change. Changing a threshold
    /// discards the accumulated rate limiter state of every tracked client.
    pub fn admin_reconfigure(
        &self,
        params: TrafficControlReconfigParams,
    ) -> Result<TrafficControlReconfigParams, IotaError> {
        let TrafficControlReconfigParams {
            error_threshold,
            spam_threshold,
            dry_run,
        } = params;
        let updates = [
            (
                error_threshold,
                self.tally_state().map(|state| state.error_policy.as_ref()),
                &self.metrics.error_client_threshold,
                "error",
            ),
            (
                spam_threshold,
                self.tally_state().map(|state| state.spam_policy.as_ref()),
                &self.metrics.spam_client_threshold,
                "spam",
            ),
        ];
        // Validate the whole request first, so a rejected one applies nothing.
        for (threshold, policy, _, kind) in updates {
            if let Some(threshold) = threshold {
                validate_threshold(policy, threshold, kind)?;
            }
        }
        for (threshold, policy, gauge, _) in updates {
            if let (Some(threshold), Some(policy)) = (threshold, policy) {
                policy.set_client_threshold(threshold);
                gauge.set(threshold as i64);
            }
        }
        if let Some(dry_run) = dry_run {
            self.metrics.dry_run_enabled.set(dry_run as i64);
            self.dry_run.store(dry_run, Ordering::Relaxed);
        }

        Ok(self.get_current_state())
    }

    /// Charges the tally against the spam and error policies, applying any
    /// resulting block before returning. No-op in allowlist mode.
    pub fn tally(&self, tally: TrafficTally) {
        let Some(state) = self.tally_state() else {
            return;
        };
        self.metrics.tallies.inc();
        if tally.spam_weight.is_sampled() && self.policy_config.spam_sample_rate.is_sampled() {
            let response = state.spam_policy.charge(&tally);
            self.metrics.tally_handled.inc();
            self.apply_policy_response(response, state, |delegation| {
                delegation.delegate_spam_blocking
            });
        }
        if let Some((error_weight, error_type)) = &tally.error_info {
            if error_weight.is_sampled() {
                self.metrics
                    .tally_error_types
                    .with_label_values(&[error_type.as_str()])
                    .inc();
                let response = state.error_policy.charge(&tally);
                self.metrics.error_tally_handled.inc();
                self.apply_policy_response(response, state, |delegation| {
                    delegation.delegate_error_blocking
                });
            }
        }
    }

    /// Blocks the breaching clients, either locally or by handing them to the
    /// remote firewall when the policy delegates that kind of blocking.
    fn apply_policy_response(
        &self,
        response: PolicyResponse,
        state: &TallyState,
        delegates: impl FnOnce(&FirewallDelegation) -> bool,
    ) {
        if response.block_client.is_none() && response.block_proxied_client.is_none() {
            return;
        }
        // The firewall must receive no blocks during a drain or a dry run.
        match state.firewall_delegation.as_ref().filter(|delegation| {
            delegates(delegation)
                && !state.drainfile_present.load(Ordering::Relaxed)
                && !self.dry_run.load(Ordering::Relaxed)
        }) {
            Some(delegation) => self.delegate_policy_response(&response, state, delegation),
            None => block_locally(
                &response,
                &self.policy_config,
                &state.blocklists,
                &self.metrics,
            ),
        }
    }

    fn delegate_policy_response(
        &self,
        response: &PolicyResponse,
        state: &TallyState,
        delegation: &FirewallDelegation,
    ) {
        let blocks: Vec<_> =
            block_addresses(response, &self.policy_config, delegation.destination_port)
                .into_iter()
                .filter(|block| delegation.pending.lock().insert(block.client))
                .collect();
        if blocks.is_empty() {
            return;
        }
        let dropped = match delegation.sender.try_send(blocks) {
            Ok(()) => return,
            Err(TrySendError::Full(dropped)) => {
                // Not logged: it recurs on every request of a sustained breach.
                self.metrics.firewall_delegation_overflow.inc();
                dropped
            }
            Err(TrySendError::Closed(dropped)) => {
                warn!("Firewall delegation queue closed unexpectedly");
                dropped
            }
        };
        release_pending(
            &delegation.pending,
            dropped.into_iter().map(|block| block.client),
        );
        block_locally(
            response,
            &self.policy_config,
            &state.blocklists,
            &self.metrics,
        );
    }

    /// Handle check with dry-run mode considered
    pub fn check(&self, client: &Option<IpAddr>, proxied_client: &Option<IpAddr>) -> bool {
        let dry_run = self.dry_run.load(Ordering::Relaxed);
        let allowed = match &self.acl {
            Acl::Allowlist(allowlist) => client.is_none_or(|client| allowlist.contains(&client)),
            Acl::Tally(state) => check_blocklists(&state.blocklists, client, proxied_client),
        };
        match (allowed, dry_run) {
            (true, _) => true,
            (false, true) => {
                debug!("Dry run mode: Blocked request from client {:?}", client);
                self.metrics.num_dry_run_blocked_requests.inc();
                true
            }
            (false, false) => {
                debug!("Blocked request from client {:?}", client);
                self.metrics.requests_blocked_at_protocol.inc();
                false
            }
        }
    }
}

fn parse_allowlist(allow_list: &[String]) -> Vec<IpAddr> {
    allow_list
        .iter()
        .map(|ip_str| {
            parse_ip(ip_str)
                .unwrap_or_else(|| fatal!("Failed to parse allowlist IP address: {ip_str:?}"))
        })
        .collect()
}

/// Builds the tallying state and spawns its background tasks. Must be called
/// from within a tokio runtime.
fn spawn_tally_state(
    policy_config: &PolicyConfig,
    metrics: &Arc<TrafficControllerMetrics>,
    fw_config: Option<&RemoteFirewallConfig>,
) -> TallyState {
    let blocklists = Blocklists {
        clients: Arc::new(DashMap::new()),
        proxied_clients: Arc::new(DashMap::new()),
    };
    let spam_policy = Arc::new(TrafficControlPolicy::from_policy_type(
        &policy_config.spam_policy_type,
        policy_config.connection_blocklist_ttl_sec,
    ));
    let error_policy = Arc::new(TrafficControlPolicy::from_policy_type(
        &policy_config.error_policy_type,
        policy_config.connection_blocklist_ttl_sec,
    ));
    let drainfile_present = Arc::new(AtomicBool::new(false));
    let shutdown = CancellationToken::new();
    let mut firewall_delegation = None;

    if let Some(fw_config) = fw_config {
        // An unreadable path counts as a drain, so the node does not delegate
        // while it cannot tell.
        let present = fw_config.drain_path.try_exists().unwrap_or(true);
        drainfile_present.store(present, Ordering::Relaxed);
        metrics.deadmans_switch_enabled.set(present as i64);

        let (sender, receiver) = mpsc::channel(FIREWALL_DELEGATION_QUEUE_SIZE);
        let pending = Arc::new(Mutex::new(HashSet::new()));
        firewall_delegation = Some(FirewallDelegation {
            sender,
            pending: pending.clone(),
            destination_port: fw_config.destination_port,
            delegate_spam_blocking: fw_config.delegate_spam_blocking,
            delegate_error_blocking: fw_config.delegate_error_blocking,
        });
        let nodefw_client = NodeFWClient::new(fw_config.remote_fw_url.clone());
        let delegation_blocklists = blocklists.clone();
        let delegation_metrics = metrics.clone();
        spawn_monitored_task!(run_firewall_delegation_loop(
            receiver,
            pending,
            nodefw_client,
            delegation_blocklists,
            delegation_metrics
        ));

        let deadmans_switch_fw_config = fw_config.clone();
        let deadmans_switch_drainfile = drainfile_present.clone();
        let deadmans_switch_metrics = metrics.clone();
        let deadmans_switch_shutdown = shutdown.clone();
        spawn_monitored_task!(run_deadmans_switch_loop(
            deadmans_switch_fw_config,
            deadmans_switch_drainfile,
            deadmans_switch_metrics,
            deadmans_switch_shutdown
        ));
    }

    let clear_loop_blocklists = blocklists.clone();
    let clear_loop_metrics = metrics.clone();
    let clear_loop_shutdown = shutdown.clone();
    spawn_monitored_task!(run_clear_blocklists_loop(
        clear_loop_blocklists,
        clear_loop_metrics,
        clear_loop_shutdown
    ));

    TallyState {
        spam_policy,
        error_policy,
        blocklists,
        firewall_delegation,
        drainfile_present,
        shutdown,
    }
}

fn validate_threshold(
    policy: Option<&TrafficControlPolicy>,
    threshold: u64,
    kind: &str,
) -> Result<(), IotaError> {
    let Some(policy) = policy else {
        return Err(IotaError::InvalidAdminRequest(format!(
            "Cannot reconfigure {kind} policy threshold in allowlist mode"
        )));
    };
    if threshold > MAX_CLIENT_THRESHOLD {
        return Err(IotaError::InvalidAdminRequest(format!(
            "Threshold {threshold} exceeds the maximum of {MAX_CLIENT_THRESHOLD}"
        )));
    }
    if policy.client_threshold().is_none() {
        return Err(IotaError::InvalidAdminRequest(
            "Unsupported prior policy type during traffic control reconfiguration".to_string(),
        ));
    }
    Ok(())
}

/// Reports the thresholds the policies enforce.
fn set_policy_config_metrics(
    state: &TallyState,
    policy_config: &PolicyConfig,
    metrics: &TrafficControllerMetrics,
) {
    if let Some(threshold) = state.spam_policy.client_threshold() {
        metrics.spam_client_threshold.set(threshold as i64);
    }
    if let Some(threshold) = state.error_policy.client_threshold() {
        metrics.error_client_threshold.set(threshold as i64);
    }
    if let PolicyType::FreqThreshold(config) = &policy_config.spam_policy_type {
        metrics
            .spam_proxied_client_threshold
            .set(config.proxied_client_threshold.min(MAX_CLIENT_THRESHOLD) as i64);
    }
    if let PolicyType::FreqThreshold(config) = &policy_config.error_policy_type {
        metrics
            .error_proxied_client_threshold
            .set(config.proxied_client_threshold.min(MAX_CLIENT_THRESHOLD) as i64);
    }
}

/// Returns true if neither client is blocked.
fn check_blocklists(
    blocklists: &Blocklists,
    client: &Option<IpAddr>,
    proxied_client: &Option<IpAddr>,
) -> bool {
    !blocked(client, &blocklists.clients) && !blocked(proxied_client, &blocklists.proxied_clients)
}

fn blocked(client: &Option<IpAddr>, blocklist: &Blocklist) -> bool {
    client.is_some_and(|client| {
        blocklist
            .get(&client)
            .is_some_and(|expiration| SystemTime::now() < *expiration)
    })
}

/// The client to block and the TTL of that block, for the direct and the
/// proxied client in that order.
fn blocks(response: &PolicyResponse, policy_config: &PolicyConfig) -> [(Option<IpAddr>, u64); 2] {
    [
        (
            response.block_client,
            policy_config.connection_blocklist_ttl_sec,
        ),
        (
            response.block_proxied_client,
            policy_config.proxy_blocklist_ttl_sec,
        ),
    ]
}

fn block_locally(
    response: &PolicyResponse,
    policy_config: &PolicyConfig,
    blocklists: &Blocklists,
    metrics: &TrafficControllerMetrics,
) {
    let targets = [
        (&blocklists.clients, &metrics.connection_ip_blocklist_len),
        (&blocklists.proxied_clients, &metrics.proxy_ip_blocklist_len),
    ];
    for ((client, ttl_secs), (blocklist, len_gauge)) in
        blocks(response, policy_config).into_iter().zip(targets)
    {
        let Some(client) = client else { continue };
        insert_block(blocklist, len_gauge, client, ttl_secs);
    }
}

/// Blocks a client for `ttl_secs`, counting it only when it was not already
/// blocked so that the gauge matches the blocklist length.
fn insert_block(blocklist: &Blocklist, len_gauge: &IntGauge, client: IpAddr, ttl_secs: u64) {
    if blocklist
        .insert(client, SystemTime::now() + Duration::from_secs(ttl_secs))
        .is_none()
    {
        debug!("Adding client {client:?} to blocklist");
        len_gauge.inc();
    }
}

fn block_addresses(
    response: &PolicyResponse,
    policy_config: &PolicyConfig,
    destination_port: u16,
) -> Vec<DelegatedBlock> {
    blocks(response, policy_config)
        .into_iter()
        .zip([false, true])
        .filter_map(|((client, ttl), proxied)| {
            let client = client?;
            debug!("Delegating blocking of client {client:?} to firewall");
            Some(DelegatedBlock {
                client,
                address: BlockAddress {
                    source_address: client.to_string(),
                    destination_port,
                    ttl,
                },
                proxied,
            })
        })
        .collect()
}

/// Releases clients whose block never reached the firewall, which would
/// otherwise never enqueue a block again.
fn release_pending(pending: &Mutex<HashSet<IpAddr>>, clients: impl IntoIterator<Item = IpAddr>) {
    let mut pending = pending.lock();
    for client in clients {
        pending.remove(&client);
    }
}

/// Waits for the next tick of a background loop, returning false once the last
/// controller holding the loop's state has been dropped.
async fn tick(interval: Duration, shutdown: &CancellationToken) -> bool {
    tokio::select! {
        _ = tokio::time::sleep(interval) => true,
        _ = shutdown.cancelled() => false,
    }
}

/// Drops expired blocklist entries and refreshes the length gauges.
async fn run_clear_blocklists_loop(
    blocklists: Blocklists,
    metrics: Arc<TrafficControllerMetrics>,
    shutdown: CancellationToken,
) {
    while tick(CLEAR_BLOCKLIST_INTERVAL, &shutdown).await {
        let now = SystemTime::now();
        blocklists.clients.retain(|_, expiration| now < *expiration);
        blocklists
            .proxied_clients
            .retain(|_, expiration| now < *expiration);
        metrics
            .connection_ip_blocklist_len
            .set(blocklists.clients.len() as i64);
        metrics
            .proxy_ip_blocklist_len
            .set(blocklists.proxied_clients.len() as i64);
    }
}

/// Posts delegated blocks to the remote firewall, off the request path.
async fn run_firewall_delegation_loop(
    mut receiver: mpsc::Receiver<Vec<DelegatedBlock>>,
    pending: Arc<Mutex<HashSet<IpAddr>>>,
    node_fw_client: NodeFWClient,
    blocklists: Blocklists,
    metrics: Arc<TrafficControllerMetrics>,
) {
    while let Some(batch) = receiver.recv().await {
        let addresses: Vec<_> = batch.iter().map(|block| block.address.clone()).collect();
        metrics
            .blocks_delegated_to_firewall
            .inc_by(addresses.len() as u64);
        if let Err(err) = node_fw_client
            .block_addresses(BlockAddresses { addresses })
            .await
        {
            metrics.firewall_delegation_request_fail.inc();
            warn!("Failed to delegate blocklist to firewall: {err}");
            for block in &batch {
                let (blocklist, len_gauge) = if block.proxied {
                    (&blocklists.proxied_clients, &metrics.proxy_ip_blocklist_len)
                } else {
                    (&blocklists.clients, &metrics.connection_ip_blocklist_len)
                };
                insert_block(blocklist, len_gauge, block.client, block.address.ttl);
            }
        }
        release_pending(&pending, batch.into_iter().map(|block| block.client));
    }
    info!("TrafficController firewall delegation queue closed by all senders");
}

/// Drains the firewall if no tallies arrive for the configured timeout, so it
/// stops blocking on stale state.
async fn run_deadmans_switch_loop(
    fw_config: RemoteFirewallConfig,
    drainfile_present: Arc<AtomicBool>,
    metrics: Arc<TrafficControllerMetrics>,
    shutdown: CancellationToken,
) {
    let timeout = Duration::from_secs(fw_config.drain_timeout_secs);
    let mut last_tallies = metrics.tallies.get();
    let mut last_tally_at = Instant::now();
    while tick(DEADMANS_SWITCH_POLL_INTERVAL, &shutdown).await {
        // The operator can add or remove the drain file at any time, so
        // delegation restarts after a drain. An I/O error keeps the last known
        // state, because `exists` cannot tell an error from a removal.
        match fw_config.drain_path.try_exists() {
            Ok(present) => {
                drainfile_present.store(present, Ordering::Relaxed);
                metrics.deadmans_switch_enabled.set(present as i64);
            }
            Err(err) => warn!("Failed to read the nodefw drain file: {err}"),
        }
        let tallies = metrics.tallies.get();
        if tallies != last_tallies {
            last_tallies = tallies;
            last_tally_at = Instant::now();
            continue;
        }
        if last_tally_at.elapsed() < timeout || drainfile_present.load(Ordering::Relaxed) {
            continue;
        }
        error!(
            "No traffic tallies received in {} seconds.",
            timeout.as_secs()
        );
        warn!("Draining Node firewall.");
        if let Err(err) = File::create(&fw_config.drain_path) {
            error!("Failed to create node firewall drain file: {err}");
            continue;
        }
        drainfile_present.store(true, Ordering::Relaxed);
        metrics.deadmans_switch_enabled.set(1);
    }
}

#[derive(Debug, Clone, Default)]
pub struct TrafficSimMetrics {
    pub num_requests: u64,
    pub num_blocked: u64,
    pub abs_time_to_first_block: Option<Duration>,
    pub num_blocklist_adds: u64,
}

impl Add for TrafficSimMetrics {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            num_requests: self.num_requests + other.num_requests,
            num_blocked: self.num_blocked + other.num_blocked,
            abs_time_to_first_block: match (
                self.abs_time_to_first_block,
                other.abs_time_to_first_block,
            ) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (Some(a), None) => Some(a),
                (None, Some(b)) => Some(b),
                (None, None) => None,
            },
            num_blocklist_adds: self.num_blocklist_adds + other.num_blocklist_adds,
        }
    }
}

pub struct TrafficSim;

impl TrafficSim {
    pub async fn run(
        policy: PolicyConfig,
        num_clients: u8,
        per_client_tps: usize,
        duration: Duration,
    ) -> TrafficSimMetrics {
        assert!(
            per_client_tps <= 10_000,
            "per_client_tps must be less than 10,000. For higher values, increase num_clients"
        );
        assert!(num_clients < 20, "num_clients must be less than 20");
        assert!(num_clients > 0);
        assert!(per_client_tps > 0);
        assert!(duration.as_secs() > 0);

        let controller = TrafficController::init_for_test(policy, None);
        let tasks = (0..num_clients).map(|task_num| {
            tokio::spawn(Self::run_single_client(
                controller.clone(),
                duration,
                task_num,
                per_client_tps,
            ))
        });

        futures::future::join_all(tasks).await.into_iter().fold(
            TrafficSimMetrics::default(),
            |acc, run_client_ret| match run_client_ret {
                Ok(metrics) => acc + metrics,
                Err(err) => {
                    error!("Error running traffic sim client: {:?}", err);
                    acc
                }
            },
        )
    }

    async fn run_single_client(
        controller: TrafficController,
        duration: Duration,
        task_num: u8,
        per_client_tps: usize,
    ) -> TrafficSimMetrics {
        // Do an initial sleep for a random amount of time to smooth
        // out the traffic. This shouldn't be strictly necessary and
        // we can remove if we want more determinism
        let sleep_time = Duration::from_micros(rand::thread_rng().gen_range(0..100));
        tokio::time::sleep(sleep_time).await;

        // collectors
        let mut num_requests = 0;
        let mut num_blocked = 0;
        let mut time_to_first_block = None;
        let mut num_blocklist_adds = 0;
        // state variables
        let mut currently_blocked = false;
        let start = Instant::now();

        // we use ticker instead of sleep to be as close to the target TPS as possible,
        let sleep_time = Duration::from_micros(1_000_000 / per_client_tps as u64);
        let mut interval_ticker = time::interval(sleep_time);

        while start.elapsed() < duration {
            let client = Some(IpAddr::V4(Ipv4Addr::new(127, 0, 0, task_num)));
            let allowed = controller.check(&client, &None);
            if allowed {
                currently_blocked = false;
                controller.tally(TrafficTally::new(
                    client,
                    // TODO add proxy IP for testing
                    None,
                    // TODO add weight adjustments
                    None,
                    Weight::one(),
                ));
            } else {
                if !currently_blocked {
                    currently_blocked = true;
                    num_blocklist_adds += 1;
                    if time_to_first_block.is_none() {
                        time_to_first_block = Some(start.elapsed());
                    }
                }
                num_blocked += 1;
            }
            num_requests += 1;

            interval_ticker.tick().await;
        }
        TrafficSimMetrics {
            num_requests,
            num_blocked,
            abs_time_to_first_block: time_to_first_block,
            num_blocklist_adds,
        }
    }
}

pub fn parse_ip(ip: &str) -> Option<IpAddr> {
    ip.parse::<IpAddr>().ok().or_else(|| {
        ip.parse::<SocketAddr>()
            .ok()
            .map(|socket_addr| socket_addr.ip())
            .or_else(|| {
                error!("Failed to parse value of {:?} to ip address or socket.", ip,);
                None
            })
    })
}

/// Outcome of resolving the client IP for an incoming request.
#[derive(Debug)]
pub enum ClientIpStatus {
    Ok(IpAddr),
    /// `SocketAddr` source but the IO type did not expose a remote address
    /// (e.g. Unix sockets, custom transports). In tests this is usually a
    /// programming error; in production it usually means a misconfigured
    /// transport.
    SocketAddrMissing,
    /// `XForwardedFor` source but no `x-forwarded-for` header on the request.
    XForwardedForHeaderMissing,
    /// `XForwardedFor` source but the header value was not valid UTF-8.
    XForwardedForInvalidUtf8,
    /// `XForwardedFor` configured with `num_hops == 0` (operator misconfig).
    XForwardedForZeroHops,
    /// `XForwardedFor` configured with `expected` hops but the header
    /// only had `actual` entries.
    XForwardedForConfigMismatch {
        expected: usize,
        actual: usize,
    },
    /// `XForwardedFor` header was present and well-formed but the chosen hop
    /// position did not parse as an IP address.
    XForwardedForUnparsable,
}

/// Resolve the client IP for an incoming request.
pub fn get_client_ip(
    headers: &http::HeaderMap,
    remote_addr: Option<SocketAddr>,
    source: &ClientIdSource,
) -> ClientIpStatus {
    match source {
        ClientIdSource::SocketAddr => match remote_addr {
            Some(addr) => ClientIpStatus::Ok(addr.ip()),
            None => ClientIpStatus::SocketAddrMissing,
        },
        ClientIdSource::XForwardedFor(num_hops) => {
            let header = match headers
                .get("x-forwarded-for")
                .or_else(|| headers.get("X-Forwarded-For"))
            {
                Some(h) => h,
                None => return ClientIpStatus::XForwardedForHeaderMissing,
            };
            let value = match header.to_str() {
                Ok(v) => v,
                Err(_) => return ClientIpStatus::XForwardedForInvalidUtf8,
            };
            if *num_hops == 0 {
                return ClientIpStatus::XForwardedForZeroHops;
            }
            let contents: Vec<&str> = value.split(',').map(str::trim).collect();
            if contents.len() < *num_hops {
                return ClientIpStatus::XForwardedForConfigMismatch {
                    expected: *num_hops,
                    actual: contents.len(),
                };
            }
            match parse_ip(contents[contents.len() - num_hops]) {
                Some(ip) => ClientIpStatus::Ok(ip),
                None => ClientIpStatus::XForwardedForUnparsable,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{net::Ipv4Addr, path::PathBuf};

    use super::*;

    const CLIENT: IpAddr = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));

    /// The policy that the request breaches.
    #[derive(Clone, Copy)]
    enum PolicyKind {
        Spam,
        Error,
    }

    /// Where the block for one breaching request went.
    #[derive(Debug, PartialEq)]
    struct Outcome {
        blocked_locally: i64,
        delegated: u64,
    }

    /// Blocks the direct client on every tally of the given policy.
    fn policy_config(dry_run: bool, kind: PolicyKind) -> PolicyConfig {
        let error = matches!(kind, PolicyKind::Error);
        let blocking_policy = PolicyType::TestNConnIP(1);
        PolicyConfig {
            spam_policy_type: if error {
                PolicyType::NoOp
            } else {
                blocking_policy.clone()
            },
            error_policy_type: if error {
                blocking_policy
            } else {
                PolicyType::NoOp
            },
            spam_sample_rate: Weight::one(),
            // A TTL of zero would expire the block at once.
            connection_blocklist_ttl_sec: 120,
            dry_run,
            ..Default::default()
        }
    }

    /// Delegates both policies. No server listens on the firewall URL, thus the
    /// metrics show if the node delegates a block.
    fn fw_config(drain_path: PathBuf) -> RemoteFirewallConfig {
        RemoteFirewallConfig {
            remote_fw_url: "http://127.0.0.1:1".to_string(),
            destination_port: 8080,
            delegate_spam_blocking: true,
            delegate_error_blocking: true,
            drain_path,
            drain_timeout_secs: 300,
        }
    }

    fn breach(kind: PolicyKind) -> TrafficTally {
        let error_info =
            matches!(kind, PolicyKind::Error).then(|| (Weight::one(), "error".to_string()));
        TrafficTally::new(Some(CLIENT), None, error_info, Weight::one())
    }

    /// Tallies one breaching request against a controller whose firewall config
    /// delegates both policies.
    async fn tally_one_breach(dry_run: bool, kind: PolicyKind) -> Outcome {
        let (_tmp_dir, controller) = delegating_controller(dry_run, kind);
        controller.tally(breach(kind));
        wait_for_block(&controller).await
    }

    /// Makes a controller that delegates both policies. Keep the directory: the
    /// dead man's switch reads `drain_path` in it.
    fn delegating_controller(dry_run: bool, kind: PolicyKind) -> (impl Drop, TrafficController) {
        let tmp_dir = iota_common::tempdir();
        let controller = TrafficController::init_for_test(
            policy_config(dry_run, kind),
            Some(fw_config(tmp_dir.path().join("drain"))),
        );
        (tmp_dir, controller)
    }

    /// Reports where the block of the last tally went. A local block lands
    /// before `tally` returns, so the count is read before the delegation loop
    /// gets to run. A delegated block is counted once that loop picks it up.
    async fn wait_for_block(controller: &TrafficController) -> Outcome {
        let metrics = &controller.metrics;
        let blocked_locally = metrics.connection_ip_blocklist_len.get();
        for _ in 0..100 {
            let delegated = metrics.blocks_delegated_to_firewall.get();
            if blocked_locally > 0 || delegated > 0 {
                return Outcome {
                    blocked_locally,
                    delegated,
                };
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("no block was recorded in one second");
    }

    #[tokio::test]
    async fn test_dry_run_reports_the_block_it_does_not_apply() {
        let (_tmp_dir, controller) = delegating_controller(true, PolicyKind::Spam);
        controller.tally(breach(PolicyKind::Spam));
        assert_eq!(
            wait_for_block(&controller).await,
            Outcome {
                blocked_locally: 1,
                delegated: 0,
            }
        );

        // Dry run lets the request through, but it counts the client that the
        // node would block.
        assert!(controller.check(&Some(CLIENT), &None));
        assert_eq!(controller.metrics.num_dry_run_blocked_requests.get(), 1);
    }

    #[tokio::test]
    async fn test_the_admin_api_turns_dry_run_off_at_once() {
        let (_tmp_dir, controller) = delegating_controller(true, PolicyKind::Spam);
        controller.tally(breach(PolicyKind::Spam));
        assert_eq!(wait_for_block(&controller).await.delegated, 0);

        controller
            .admin_reconfigure(TrafficControlReconfigParams {
                error_threshold: None,
                spam_threshold: None,
                dry_run: Some(false),
            })
            .expect("the request changes only the dry-run flag");

        // The next tally must read the new value, not the value at startup.
        for _ in 0..100 {
            controller.tally(breach(PolicyKind::Spam));
            if controller.metrics.blocks_delegated_to_firewall.get() > 0 {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("the node delegated no block after the admin API turned dry run off");
    }

    #[tokio::test]
    async fn test_an_unreadable_drain_path_stops_delegation() {
        // The drain path goes through a file, thus `try_exists` gives an error
        // and the node cannot tell if the firewall drains.
        let tmp_dir = iota_common::tempdir();
        let blocker = tmp_dir.path().join("blocker");
        File::create(&blocker).expect("the file is created");
        let controller = TrafficController::init_for_test(
            policy_config(false, PolicyKind::Spam),
            Some(fw_config(blocker.join("drain"))),
        );
        controller.tally(breach(PolicyKind::Spam));

        // The node keeps the block, because the firewall possibly drains.
        assert_eq!(
            wait_for_block(&controller).await,
            Outcome {
                blocked_locally: 1,
                delegated: 0,
            }
        );
    }

    #[tokio::test]
    async fn test_delegation_restarts_when_the_drain_file_goes_away() {
        let tmp_dir = iota_common::tempdir();
        let drain_path = tmp_dir.path().join("drain");
        File::create(&drain_path).expect("the drain file is created");
        let controller = TrafficController::init_for_test(
            policy_config(false, PolicyKind::Spam),
            Some(fw_config(drain_path.clone())),
        );
        let metrics = &controller.metrics;

        // The firewall drains, thus the node blocks locally.
        controller.tally(breach(PolicyKind::Spam));
        for _ in 0..100 {
            if metrics.connection_ip_blocklist_len.get() > 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert_eq!(metrics.connection_ip_blocklist_len.get(), 1);
        assert_eq!(metrics.blocks_delegated_to_firewall.get(), 0);

        // The operator removes the drain file, thus delegation restarts.
        fs::remove_file(&drain_path).expect("the drain file is removed");
        for _ in 0..100 {
            controller.tally(breach(PolicyKind::Spam));
            if metrics.blocks_delegated_to_firewall.get() > 0 {
                return;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        panic!("the node delegated no block in ten seconds");
    }

    #[tokio::test]
    async fn test_dry_run_keeps_spam_blocks_local() {
        assert_eq!(
            tally_one_breach(true, PolicyKind::Spam).await,
            Outcome {
                blocked_locally: 1,
                delegated: 0,
            }
        );
    }

    #[tokio::test]
    async fn test_dry_run_keeps_error_blocks_local() {
        assert_eq!(
            tally_one_breach(true, PolicyKind::Error).await,
            Outcome {
                blocked_locally: 1,
                delegated: 0,
            }
        );
    }

    #[tokio::test]
    async fn test_spam_blocks_are_delegated_without_dry_run() {
        assert_eq!(
            tally_one_breach(false, PolicyKind::Spam).await,
            Outcome {
                blocked_locally: 0,
                delegated: 1,
            }
        );
    }

    #[tokio::test]
    async fn test_error_blocks_are_delegated_without_dry_run() {
        assert_eq!(
            tally_one_breach(false, PolicyKind::Error).await,
            Outcome {
                blocked_locally: 0,
                delegated: 1,
            }
        );
    }
}
