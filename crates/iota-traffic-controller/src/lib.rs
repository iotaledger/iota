// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Protocol level traffic control. Tallies are charged to the spam and error
//! policies inline, so a breaching client is blocked before
//! [`TrafficController::tally`] returns.

pub mod metrics;
pub mod policies;
pub mod sim;

use std::{
    fmt::Debug,
    net::{IpAddr, SocketAddr},
    str,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, SystemTime},
};

use dashmap::DashMap;
use iota_common::fatal;
use iota_metrics::spawn_monitored_task;
use iota_types::{
    error::IotaError,
    traffic_control::{ClientIdSource, PolicyConfig, PolicyType, TrafficControlReconfigParams},
};
use prometheus_filtered::IntGauge;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error};

use self::metrics::TrafficControllerMetrics;
use crate::policies::{MAX_CLIENT_THRESHOLD, PolicyResponse, TrafficControlPolicy, TrafficTally};

const CLEAR_BLOCKLIST_INTERVAL: Duration = Duration::from_secs(3);

type Blocklist = Arc<DashMap<IpAddr, SystemTime>>;

#[derive(Clone)]
struct Blocklists {
    clients: Blocklist,
    proxied_clients: Blocklist,
}

/// The blocklist TTLs in force, which the admin API can change at runtime.
struct BlocklistTtls {
    connection_blocklist_ttl_sec: AtomicU64,
    proxy_blocklist_ttl_sec: AtomicU64,
}

impl BlocklistTtls {
    fn from_config(policy_config: &PolicyConfig) -> Self {
        Self {
            connection_blocklist_ttl_sec: AtomicU64::new(
                policy_config.connection_blocklist_ttl_sec,
            ),
            proxy_blocklist_ttl_sec: AtomicU64::new(policy_config.proxy_blocklist_ttl_sec),
        }
    }

    fn connection_blocklist_ttl_sec(&self) -> u64 {
        self.connection_blocklist_ttl_sec.load(Ordering::Relaxed)
    }

    fn proxy_blocklist_ttl_sec(&self) -> u64 {
        self.proxy_blocklist_ttl_sec.load(Ordering::Relaxed)
    }
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
    shutdown: CancellationToken,
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
    // Read whenever a block is applied and changed by the admin API.
    blocklist_ttls: Arc<BlocklistTtls>,
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
    pub fn init(policy_config: PolicyConfig, metrics: Arc<TrafficControllerMetrics>) -> Self {
        metrics.dry_run_enabled.set(policy_config.dry_run as i64);
        let dry_run = Arc::new(AtomicBool::new(policy_config.dry_run));
        metrics
            .connection_blocklist_ttl_sec
            .set(policy_config.connection_blocklist_ttl_sec as i64);
        metrics
            .proxy_blocklist_ttl_sec
            .set(policy_config.proxy_blocklist_ttl_sec as i64);
        let blocklist_ttls = Arc::new(BlocklistTtls::from_config(&policy_config));

        let acl = match &policy_config.allow_list {
            Some(allow_list) => Acl::Allowlist(parse_allowlist(allow_list)),
            None => {
                let state = spawn_tally_state(&policy_config, &metrics);
                set_policy_config_metrics(&state, &policy_config, &metrics);
                Acl::Tally(Arc::new(state))
            }
        };
        Self {
            acl,
            policy_config: Arc::new(policy_config),
            metrics,
            dry_run,
            blocklist_ttls,
        }
    }

    pub fn init_for_test(policy_config: PolicyConfig) -> Self {
        Self::init(
            policy_config,
            Arc::new(TrafficControllerMetrics::new_for_tests()),
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
            connection_blocklist_ttl_sec: Some(self.blocklist_ttls.connection_blocklist_ttl_sec()),
            proxy_blocklist_ttl_sec: Some(self.blocklist_ttls.proxy_blocklist_ttl_sec()),
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
            connection_blocklist_ttl_sec,
            proxy_blocklist_ttl_sec,
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
        let ttls = [
            (
                connection_blocklist_ttl_sec,
                &self.blocklist_ttls.connection_blocklist_ttl_sec,
                &self.metrics.connection_blocklist_ttl_sec,
            ),
            (
                proxy_blocklist_ttl_sec,
                &self.blocklist_ttls.proxy_blocklist_ttl_sec,
                &self.metrics.proxy_blocklist_ttl_sec,
            ),
        ];
        for (ttl_sec, live_ttl_sec, gauge) in ttls {
            if let Some(ttl_sec) = ttl_sec {
                live_ttl_sec.store(ttl_sec, Ordering::Relaxed);
                gauge.set(ttl_sec as i64);
            }
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
            block_locally(
                &response,
                &self.blocklist_ttls,
                &state.blocklists,
                &self.metrics,
            );
        }
        if let Some((error_weight, error_type)) = &tally.error_info {
            if error_weight.is_sampled() {
                self.metrics
                    .tally_error_types
                    .with_label_values(&[error_type.as_str()])
                    .inc();
                let response = state.error_policy.charge(&tally);
                self.metrics.error_tally_handled.inc();
                block_locally(
                    &response,
                    &self.blocklist_ttls,
                    &state.blocklists,
                    &self.metrics,
                );
            }
        }
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
    let shutdown = CancellationToken::new();
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
fn blocks(response: &PolicyResponse, ttls: &BlocklistTtls) -> [(Option<IpAddr>, u64); 2] {
    [
        (response.block_client, ttls.connection_blocklist_ttl_sec()),
        (
            response.block_proxied_client,
            ttls.proxy_blocklist_ttl_sec(),
        ),
    ]
}

fn block_locally(
    response: &PolicyResponse,
    ttls: &BlocklistTtls,
    blocklists: &Blocklists,
    metrics: &TrafficControllerMetrics,
) {
    let targets = [
        (&blocklists.clients, &metrics.connection_ip_blocklist_len),
        (&blocklists.proxied_clients, &metrics.proxy_ip_blocklist_len),
    ];
    for ((client, ttl_secs), (blocklist, len_gauge)) in
        blocks(response, ttls).into_iter().zip(targets)
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
        entries: Vec<String>,
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
            Self::XForwardedForZeroHops { entries } => write!(
                f,
                "x-forwarded-for: 0 specified. x-forwarded-for contents: {entries:?}. Please \
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

/// The entries of the `x-forwarded-for` header, over every field of it and in
/// order. Empty when the request carries no such header.
fn forwarded_entries(headers: &http::HeaderMap) -> Vec<&[u8]> {
    headers
        .get_all("x-forwarded-for")
        .iter()
        .flat_map(|field| field.as_bytes().split(|byte| *byte == b','))
        .map(|entry| entry.trim_ascii())
        .collect()
}

/// How many proxies the `x-forwarded-for` header of a request reports, or
/// `None` when it carries no such header. Reported whatever the client-id
/// source is, so that an operator can tell a misconfigured proxy from a
/// correct one.
pub fn forwarded_hop_depth(headers: &http::HeaderMap) -> Option<usize> {
    let entries = forwarded_entries(headers);
    (!entries.is_empty()).then(|| entries.len().saturating_sub(1))
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
            // A proxy either appends to the value the client sent or adds a
            // field of its own, so a client could otherwise hide the entry its
            // proxy wrote.
            let raw_entries = forwarded_entries(headers);
            if raw_entries.is_empty() {
                return ClientIpStatus::XForwardedForHeaderMissing;
            }
            if *num_hops == 0 {
                return ClientIpStatus::XForwardedForZeroHops {
                    entries: raw_entries
                        .iter()
                        .map(|entry| String::from_utf8_lossy(entry).into_owned())
                        .collect(),
                };
            }
            if raw_entries.len() < *num_hops {
                return ClientIpStatus::XForwardedForConfigMismatch {
                    expected: *num_hops,
                    actual: raw_entries.len(),
                };
            }
            let Ok(entry) = str::from_utf8(raw_entries[raw_entries.len() - num_hops]) else {
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
    use std::net::Ipv4Addr;

    use iota_macros::sim_test;
    use iota_types::traffic_control::{FreqThresholdConfig, Weight, default_blocklist_ttl_sec};

    use super::*;

    const CLIENT: IpAddr = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));

    /// The policy that the request breaches.
    #[derive(Clone, Copy)]
    enum PolicyKind {
        Spam,
        Error,
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

    fn breach(kind: PolicyKind) -> TrafficTally {
        let error_info =
            matches!(kind, PolicyKind::Error).then(|| (Weight::one(), "error".to_string()));
        TrafficTally::new(Some(CLIENT), None, error_info, Weight::one())
    }

    #[tokio::test]
    async fn test_dry_run_reports_the_block_it_does_not_apply() {
        let controller = TrafficController::init_for_test(policy_config(true, PolicyKind::Spam));
        controller.tally(breach(PolicyKind::Spam));
        assert_eq!(controller.metrics.connection_ip_blocklist_len.get(), 1);

        // Dry run lets the request through, but it counts the client that the
        // node would block.
        assert!(controller.check(&Some(CLIENT), &None));
        assert_eq!(controller.metrics.num_dry_run_blocked_requests.get(), 1);
    }

    #[tokio::test]
    async fn test_the_admin_api_turns_dry_run_off_at_once() {
        let controller = TrafficController::init_for_test(policy_config(true, PolicyKind::Spam));
        controller.tally(breach(PolicyKind::Spam));
        assert!(controller.check(&Some(CLIENT), &None));

        controller
            .admin_reconfigure(TrafficControlReconfigParams {
                error_threshold: None,
                spam_threshold: None,
                dry_run: Some(false),
                connection_blocklist_ttl_sec: None,
                proxy_blocklist_ttl_sec: None,
            })
            .expect("the request changes only the dry-run flag");

        // The next check must read the new value, not the value at startup.
        assert!(!controller.check(&Some(CLIENT), &None));
    }

    #[tokio::test]
    async fn test_dry_run_reports_an_error_block_it_does_not_apply() {
        let controller = TrafficController::init_for_test(policy_config(true, PolicyKind::Error));
        controller.tally(breach(PolicyKind::Error));
        assert_eq!(controller.metrics.connection_ip_blocklist_len.get(), 1);
        assert!(controller.check(&Some(CLIENT), &None));
    }

    #[tokio::test]
    async fn test_the_admin_api_changes_the_blocklist_ttl() {
        let controller = TrafficController::init_for_test(PolicyConfig {
            spam_policy_type: PolicyType::TestNConnIP(1),
            spam_sample_rate: Weight::one(),
            connection_blocklist_ttl_sec: 0,
            dry_run: false,
            ..Default::default()
        });

        // A TTL of zero expires the block at once, thus the client gets through.
        controller.tally(breach(PolicyKind::Spam));
        assert!(controller.check(&Some(CLIENT), &None));

        controller
            .admin_reconfigure(TrafficControlReconfigParams {
                error_threshold: None,
                spam_threshold: None,
                dry_run: None,
                connection_blocklist_ttl_sec: Some(120),
                proxy_blocklist_ttl_sec: None,
            })
            .expect("the request changes only the connection blocklist TTL");

        // The next block must take the new TTL, not the TTL at startup.
        controller.tally(breach(PolicyKind::Spam));
        assert!(!controller.check(&Some(CLIENT), &None));
        assert_eq!(controller.metrics.connection_blocklist_ttl_sec.get(), 120);
    }

    /// The tallies the shipped default policy tolerates from one client before
    /// its error policy blocks that client.
    fn default_policy_error_budget() -> u64 {
        let PolicyType::FreqThreshold(config) =
            PolicyConfig::default_dos_protection_policy().error_policy_type
        else {
            panic!("the default policy rate limits errors");
        };
        config.client_threshold * config.burst_secs
    }

    #[tokio::test]
    async fn test_the_default_policy_blocks_a_breaching_client_without_dry_run() {
        let controller = TrafficController::init_for_test(PolicyConfig {
            dry_run: false,
            ..PolicyConfig::default_dos_protection_policy()
        });
        let budget = default_policy_error_budget();

        // Within its budget the client is never blocked.
        for _ in 0..budget {
            controller.tally(breach(PolicyKind::Error));
        }
        assert!(controller.check(&Some(CLIENT), &None));

        // Over the budget the shipped blocklist TTL keeps the block in place,
        // thus a later check rejects the client. The bound leaves room for the
        // cells the limiter replenishes while the test runs.
        for _ in 0..budget {
            controller.tally(breach(PolicyKind::Error));
            if !controller.check(&Some(CLIENT), &None) {
                return;
            }
        }
        panic!(
            "the default policy blocked no client after {} error tallies",
            2 * budget
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
        let controller = TrafficController::init_for_test(PolicyConfig {
            spam_policy_type: freq_threshold(100),
            error_policy_type: freq_threshold(50),
            dry_run: false,
            ..Default::default()
        });
        // The spam threshold is too large. The error threshold, the dry-run
        // flag and the TTLs are valid, but the controller must apply none of
        // them.
        let result = controller.admin_reconfigure(TrafficControlReconfigParams {
            error_threshold: Some(10),
            spam_threshold: Some(MAX_CLIENT_THRESHOLD + 1),
            dry_run: Some(true),
            connection_blocklist_ttl_sec: Some(5),
            proxy_blocklist_ttl_sec: Some(5),
        });

        assert!(matches!(result, Err(IotaError::InvalidAdminRequest(_))));
        let state = controller.get_current_state();
        assert_eq!(state.error_threshold, Some(50));
        assert_eq!(state.spam_threshold, Some(100));
        assert_eq!(state.dry_run, Some(false));
        assert_eq!(
            state.connection_blocklist_ttl_sec,
            Some(default_blocklist_ttl_sec())
        );
        assert_eq!(
            state.proxy_blocklist_ttl_sec,
            Some(default_blocklist_ttl_sec())
        );
    }

    /// The headers of a request carrying `value` as its `x-forwarded-for`
    /// header.
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
    fn the_hop_depth_survives_a_byte_the_client_sent() {
        assert_eq!(forwarded_hop_depth(&forwarded(b"\x80, 10.0.0.1")), Some(1));
        assert_eq!(forwarded_hop_depth(&forwarded(b"10.0.0.1")), Some(0));
        assert_eq!(forwarded_hop_depth(&http::HeaderMap::new()), None);
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
        let ClientIpStatus::XForwardedForZeroHops { entries } =
            get_client_ip(&headers, None, &ClientIdSource::XForwardedFor(0))
        else {
            panic!("zero hops names no client");
        };
        // The operator counts the entries after their own address to get the
        // hop count, so every entry has to be reported, in order.
        assert_eq!(entries, vec!["1.2.3.4", "10.0.0.1"]);
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
        let controller = TrafficController::init_for_test(PolicyConfig {
            allow_list: allow_list.clone(),
            dry_run: false,
            ..Default::default()
        });
        assert!(!controller.check(&None, &None));
        assert_eq!(controller.metrics.requests_blocked_at_protocol.get(), 1);
        assert_eq!(controller.metrics.unresolved_client_requests.get(), 1);

        // Dry run reports the refusal without applying it.
        let controller = TrafficController::init_for_test(PolicyConfig {
            allow_list,
            dry_run: true,
            ..Default::default()
        });
        assert!(controller.check(&None, &None));
        assert_eq!(controller.metrics.num_dry_run_blocked_requests.get(), 1);
    }

    #[tokio::test]
    async fn rate_limiting_admits_a_request_with_an_unresolved_client_ip() {
        let controller = TrafficController::init_for_test(policy_config(false, PolicyKind::Spam));
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
        let controller = TrafficController::init_for_test(PolicyConfig {
            allow_list: Some(vec!["10.0.0.9".to_string(), "10.0.0.1".to_string()]),
            dry_run: false,
            ..Default::default()
        });

        for client in ["10.0.0.9", "10.0.0.1"] {
            let client: IpAddr = client.parse().unwrap();
            assert!(controller.check(&Some(client), &None));
        }
        let stranger = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 5));
        assert!(!controller.check(&Some(stranger), &None));
    }
}
