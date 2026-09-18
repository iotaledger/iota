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
pub mod sim;

use std::{
    collections::HashSet,
    fmt::Debug,
    fs,
    net::{IpAddr, SocketAddr},
    str,
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
        TrafficControlReconfigParams,
    },
};
use parking_lot::Mutex;
use prometheus_filtered::IntGauge;
use tokio::sync::{mpsc, mpsc::error::TrySendError};
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
    /// only be populated once at initialization, and stays sorted so
    /// that `check` can binary-search it.
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

    /// Handle check with dry-run mode considered. A request whose client IP the
    /// node could not resolve is refused in allowlist mode, and admitted in the
    /// rate-limiting modes, where it is charged to no client.
    pub fn check(&self, client: &Option<IpAddr>, proxied_client: &Option<IpAddr>) -> bool {
        let dry_run = self.dry_run.load(Ordering::Relaxed);
        if client.is_none() {
            self.metrics.unresolved_client_requests.inc();
        }
        let allowed = match &self.acl {
            // An allowlist admits the clients it names, and a request with no
            // resolved client IP is none of them.
            Acl::Allowlist(allowlist) => {
                client.is_some_and(|client| allowlist.binary_search(&client).is_ok())
            }
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

/// Returns the allowlist sorted, so that `check` can binary-search it.
fn parse_allowlist(allow_list: &[String]) -> Vec<IpAddr> {
    let mut allowlist: Vec<IpAddr> = allow_list
        .iter()
        .map(|ip_str| {
            parse_ip(ip_str)
                .unwrap_or_else(|| fatal!("Failed to parse allowlist IP address: {ip_str:?}"))
        })
        .collect();
    allowlist.sort_unstable();
    allowlist
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
        metrics.rate_limiter_evictions.clone(),
    ));
    let error_policy = Arc::new(TrafficControlPolicy::from_policy_type(
        &policy_config.error_policy_type,
        policy_config.connection_blocklist_ttl_sec,
        metrics.rate_limiter_evictions.clone(),
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
    /// `XForwardedFor` source but the entry this node selects was not valid
    /// UTF-8. Bytes in the rest of the header do not reach this case.
    XForwardedForInvalidUtf8,
    /// `XForwardedFor` configured with `num_hops == 0` (operator misconfig).
    /// Carries the header entries, which the operator counts to get the hop
    /// count.
    XForwardedForZeroHops {
        contents: Vec<String>,
    },
    /// `XForwardedFor` configured with `expected` hops but the header
    /// only had `actual` entries.
    XForwardedForConfigMismatch {
        expected: usize,
        actual: usize,
    },
    /// `XForwardedFor` source but the entry this node selects did not parse as
    /// an IP address. Entries the node does not select do not reach this case.
    XForwardedForUnparsable,
}

/// Reports what the node read, in the words an operator needs to act on it.
/// The three servers share this text, and the hop-count procedure in the
/// operator guide reads the entries out of the zero-hop line.
impl std::fmt::Display for ClientIpStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ok(client) => write!(f, "The client IP is {client}."),
            Self::SocketAddrMissing => write!(
                f,
                "The request carries no peer address. Check the transport, or use the \
                `x-forwarded-for` client-id-source if a proxy serves this node."
            ),
            Self::XForwardedForHeaderMissing => write!(
                f,
                "The request carries no x-forwarded-for header, although this node reads the \
                client IP from that header. The request reached the node without its proxy."
            ),
            Self::XForwardedForInvalidUtf8 => write!(
                f,
                "The x-forwarded-for entry this node selects is not valid UTF-8."
            ),
            Self::XForwardedForZeroHops { contents } => write!(
                f,
                "x-forwarded-for: 0 specified. x-forwarded-for contents: {contents:?}. Please \
                assign a nonzero number of hops, or use the `socket-addr` client-id-source if \
                requests do not reach this node through a proxy. Until then the node reads no \
                client IP."
            ),
            Self::XForwardedForConfigMismatch { expected, actual } => write!(
                f,
                "The x-forwarded-for header holds {actual} entries, but {expected} hops are \
                configured. Please set the `x-forwarded-for` value under `client-id-source` to \
                the number of proxies in front of this node."
            ),
            Self::XForwardedForUnparsable => write!(
                f,
                "The x-forwarded-for entry this node selects is not an IP address."
            ),
        }
    }
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
            let fields = headers.get_all("x-forwarded-for");
            if fields.iter().next().is_none() {
                return ClientIpStatus::XForwardedForHeaderMissing;
            }
            // Read every field of the header, in order, and split the raw
            // bytes. A proxy either appends its entry to the value the client
            // sent or adds a field of its own, and only the entry this node
            // selects has to be valid UTF-8. Otherwise a client could hide the
            // entry its proxy wrote, with a field of its own or with a byte
            // that no entry of the node's own choosing contains.
            let entries: Vec<&[u8]> = fields
                .iter()
                .flat_map(|field| field.as_bytes().split(|byte| *byte == b','))
                .map(|entry| entry.trim_ascii())
                .collect();
            if *num_hops == 0 {
                return ClientIpStatus::XForwardedForZeroHops {
                    contents: entries
                        .iter()
                        .map(|entry| String::from_utf8_lossy(entry).into_owned())
                        .collect(),
                };
            }
            if entries.len() < *num_hops {
                return ClientIpStatus::XForwardedForConfigMismatch {
                    expected: *num_hops,
                    actual: entries.len(),
                };
            }
            let Ok(entry) = str::from_utf8(entries[entries.len() - num_hops]) else {
                return ClientIpStatus::XForwardedForInvalidUtf8;
            };
            match parse_ip(entry) {
                Some(ip) => ClientIpStatus::Ok(ip),
                None => ClientIpStatus::XForwardedForUnparsable,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{net::Ipv4Addr, path::PathBuf};

    use iota_macros::sim_test;
    use iota_types::traffic_control::{FreqThresholdConfig, Weight};

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

    fn freq_threshold(client_threshold: u64) -> PolicyType {
        PolicyType::FreqThreshold(FreqThresholdConfig {
            client_threshold,
            proxied_client_threshold: client_threshold,
            burst_secs: 5,
        })
    }

    #[sim_test]
    async fn test_rejected_reconfiguration_applies_nothing() {
        let controller = TrafficController::init_for_test(
            PolicyConfig {
                spam_policy_type: freq_threshold(100),
                error_policy_type: freq_threshold(50),
                dry_run: false,
                ..Default::default()
            },
            None,
        );
        // The spam threshold is too large. The error threshold and the dry-run
        // flag are valid, but the controller must apply neither of them.
        let result = controller.admin_reconfigure(TrafficControlReconfigParams {
            error_threshold: Some(10),
            spam_threshold: Some(MAX_CLIENT_THRESHOLD + 1),
            dry_run: Some(true),
        });

        assert!(matches!(result, Err(IotaError::InvalidAdminRequest(_))));
        let state = controller.get_current_state();
        assert_eq!(state.error_threshold, Some(50));
        assert_eq!(state.spam_threshold, Some(100));
        assert_eq!(state.dry_run, Some(false));
    }

    fn controller_with_delegation_queue(
        queue_size: usize,
    ) -> (TrafficController, mpsc::Receiver<Vec<DelegatedBlock>>) {
        // A threshold of one blocks the direct client on every tally.
        let policy_config = PolicyConfig {
            spam_policy_type: PolicyType::TestNConnIP(1),
            spam_sample_rate: Weight::one(),
            dry_run: false,
            ..Default::default()
        };
        let metrics = Arc::new(TrafficControllerMetrics::new_for_tests());
        let (sender, receiver) = mpsc::channel(queue_size);
        // The delegation loop stays unspawned. Queued blocks stay queued.
        let state = TallyState {
            spam_policy: Arc::new(TrafficControlPolicy::from_policy_type(
                &policy_config.spam_policy_type,
                policy_config.connection_blocklist_ttl_sec,
                metrics.rate_limiter_evictions.clone(),
            )),
            error_policy: Arc::new(TrafficControlPolicy::from_policy_type(
                &PolicyType::NoOp,
                policy_config.connection_blocklist_ttl_sec,
                metrics.rate_limiter_evictions.clone(),
            )),
            blocklists: Blocklists {
                clients: Arc::new(DashMap::new()),
                proxied_clients: Arc::new(DashMap::new()),
            },
            firewall_delegation: Some(FirewallDelegation {
                sender,
                pending: Arc::new(Mutex::new(HashSet::new())),
                destination_port: 8080,
                delegate_spam_blocking: true,
                delegate_error_blocking: true,
            }),
            drainfile_present: Arc::new(AtomicBool::new(false)),
            shutdown: CancellationToken::new(),
        };
        let controller = TrafficController {
            acl: Acl::Tally(Arc::new(state)),
            policy_config: Arc::new(policy_config),
            metrics,
            dry_run: Arc::new(AtomicBool::new(false)),
        };
        (controller, receiver)
    }

    fn spam(controller: &TrafficController, client: IpAddr) {
        controller.tally(TrafficTally::new(Some(client), None, None, Weight::one()));
    }

    #[test]
    fn test_delegation_queues_one_block_per_client() {
        let (controller, mut receiver) = controller_with_delegation_queue(8);
        let client = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        for _ in 0..3 {
            spam(&controller, client);
        }

        let batch = receiver.try_recv().expect("the first block is queued");
        assert_eq!(batch.len(), 1);
        assert_eq!(batch[0].client, client);
        // The client stays pending. The later tallies queue no more blocks.
        assert!(receiver.try_recv().is_err());
        // The firewall has the block. The local blocklist stays empty.
        assert!(controller.check(&Some(client), &None));
    }

    #[test]
    fn test_delegation_overflow_blocks_locally() {
        let (controller, _receiver) = controller_with_delegation_queue(1);
        let queued = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        let overflow = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2));
        spam(&controller, queued);
        spam(&controller, overflow);

        assert_eq!(controller.metrics.firewall_delegation_overflow.get(), 1);
        assert!(controller.check(&Some(queued), &None));
        // The queue is full. The controller blocks the second client locally.
        assert!(!controller.check(&Some(overflow), &None));
        // The second client is no longer pending. A new breach queues a block.
        spam(&controller, overflow);
        assert_eq!(controller.metrics.firewall_delegation_overflow.get(), 2);
    }

    /// A request carrying `value` as its `x-forwarded-for` header.
    fn forwarded(value: &[u8]) -> http::HeaderMap {
        let mut headers = http::HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            http::HeaderValue::from_bytes(value).expect("a valid header value"),
        );
        headers
    }

    fn one_hop() -> ClientIdSource {
        ClientIdSource::XForwardedFor(1)
    }

    #[test]
    fn a_byte_the_client_sent_does_not_hide_the_entry_the_proxy_wrote() {
        // The client sent a byte that is not readable as text, and the proxy
        // appended its own entry after it.
        let headers = forwarded(b"\x80, 10.0.0.1");
        assert!(matches!(
            get_client_ip(&headers, None, &one_hop()),
            ClientIpStatus::Ok(client) if client == CLIENT
        ));
    }

    #[test]
    fn a_second_header_field_does_not_hide_the_entry_the_proxy_wrote() {
        // A proxy that adds a field of its own rather than appending to the
        // client's leaves the client's field first.
        let mut headers = forwarded(b"10.0.0.9");
        headers.append(
            "x-forwarded-for",
            http::HeaderValue::from_bytes(b"10.0.0.1").expect("a valid header value"),
        );
        assert!(matches!(
            get_client_ip(&headers, None, &one_hop()),
            ClientIpStatus::Ok(client) if client == CLIENT
        ));
    }

    #[test]
    fn a_client_cannot_move_the_selected_entry_by_adding_its_own() {
        let headers = forwarded(b"1.2.3.4, 5.6.7.8, 10.0.0.1");
        assert!(matches!(
            get_client_ip(&headers, None, &one_hop()),
            ClientIpStatus::Ok(client) if client == CLIENT
        ));
    }

    #[test]
    fn an_unreadable_selected_entry_resolves_no_client() {
        let headers = forwarded(b"10.0.0.1, \x80");
        assert!(matches!(
            get_client_ip(&headers, None, &one_hop()),
            ClientIpStatus::XForwardedForInvalidUtf8
        ));
    }

    #[test]
    fn a_selected_entry_that_is_not_an_address_resolves_no_client() {
        let headers = forwarded(b"10.0.0.1, not-an-address");
        assert!(matches!(
            get_client_ip(&headers, None, &one_hop()),
            ClientIpStatus::XForwardedForUnparsable
        ));
    }

    #[test]
    fn the_first_entry_is_selected_when_the_hop_count_equals_the_entry_count() {
        let headers = forwarded(b"10.0.0.1, 5.6.7.8");
        assert!(matches!(
            get_client_ip(&headers, None, &ClientIdSource::XForwardedFor(2)),
            ClientIpStatus::Ok(client) if client == CLIENT
        ));
    }

    #[test]
    fn a_header_shorter_than_the_hop_count_reports_the_mismatch() {
        let headers = forwarded(b"1.2.3.4, 10.0.0.1");
        assert!(matches!(
            get_client_ip(&headers, None, &ClientIdSource::XForwardedFor(3)),
            ClientIpStatus::XForwardedForConfigMismatch {
                expected: 3,
                actual: 2
            }
        ));
    }

    #[test]
    fn zero_hops_reports_the_header_entries() {
        let headers = forwarded(b"1.2.3.4, 10.0.0.1");
        let ClientIpStatus::XForwardedForZeroHops { contents } =
            get_client_ip(&headers, None, &ClientIdSource::XForwardedFor(0))
        else {
            panic!("zero hops names no client");
        };
        // The operator counts the entries after their own address to get the
        // hop count, so every entry has to be reported, in order.
        assert_eq!(contents, vec!["1.2.3.4", "10.0.0.1"]);
    }

    #[test]
    fn the_zero_hop_message_is_the_one_the_operator_script_reads() {
        let headers = forwarded(b"1.2.3.4, 10.0.0.1");
        let status = get_client_ip(&headers, None, &ClientIdSource::XForwardedFor(0));
        // The same pattern `setups/validator/config-traffic-control.sh` greps
        // for. The script counts the entries after the operator's own address,
        // so it needs them in order and inside one pair of brackets.
        let message = status.to_string();
        let start = message
            .find("x-forwarded-for contents: [")
            .expect("the script looks for this prefix");
        let entries = message[start..]
            .split_once("].")
            .expect("the script looks for a closing bracket and a period")
            .0;
        assert!(entries.ends_with(r#"["1.2.3.4", "10.0.0.1""#), "{message}");
    }

    #[test]
    fn an_allowlist_refuses_a_request_with_an_unresolved_client_ip() {
        let allow_list = Some(vec![CLIENT.to_string()]);
        let controller = TrafficController::init_for_test(
            PolicyConfig {
                allow_list: allow_list.clone(),
                dry_run: false,
                ..Default::default()
            },
            None,
        );
        assert!(!controller.check(&None, &None));
        assert_eq!(controller.metrics.requests_blocked_at_protocol.get(), 1);
        assert_eq!(controller.metrics.unresolved_client_requests.get(), 1);

        // Dry run reports the refusal without applying it.
        let controller = TrafficController::init_for_test(
            PolicyConfig {
                allow_list,
                dry_run: true,
                ..Default::default()
            },
            None,
        );
        assert!(controller.check(&None, &None));
        assert_eq!(controller.metrics.num_dry_run_blocked_requests.get(), 1);
    }

    #[tokio::test]
    async fn rate_limiting_admits_a_request_with_an_unresolved_client_ip() {
        let controller =
            TrafficController::init_for_test(policy_config(false, PolicyKind::Spam), None);
        // The policy blocks every client it is charged, and a request with no
        // resolved client IP is charged to none of them.
        controller.tally(breach(PolicyKind::Spam));
        assert!(!controller.check(&Some(CLIENT), &None));
        assert!(controller.check(&None, &None));
        // Only the request with no resolved client IP is counted.
        assert_eq!(controller.metrics.unresolved_client_requests.get(), 1);
    }

    #[test]
    fn an_unsorted_allowlist_still_admits_its_clients() {
        let controller = TrafficController::init_for_test(
            PolicyConfig {
                allow_list: Some(vec!["10.0.0.9".to_string(), "10.0.0.1".to_string()]),
                dry_run: false,
                ..Default::default()
            },
            None,
        );

        for client in ["10.0.0.9", "10.0.0.1"] {
            let client: IpAddr = client.parse().unwrap();
            assert!(controller.check(&Some(client), &None));
        }
        let stranger = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 5));
        assert!(!controller.check(&Some(stranger), &None));
    }
}
