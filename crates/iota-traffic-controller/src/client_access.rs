// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Decides, by IP address, whether the node serves a client's request: either
//! every client not on the allowlist is refused, or every client on the
//! blocklist is, until its block expires.

use std::{
    net::{IpAddr, Ipv6Addr},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, SystemTime},
};

use dashmap::{DashMap, mapref::entry::Entry};
use iota_common::fatal;
use iota_metrics::spawn_monitored_task;
use iota_types::traffic_control::PolicyConfig;
use prometheus_filtered::IntGauge;
use tokio::time::{Interval, MissedTickBehavior};
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

use crate::{
    block_log::{BlockReason, log_block},
    metrics::TrafficControllerMetrics,
    parse_ip,
    policies::PolicyResponse,
};

const CLEAR_BLOCKLIST_INTERVAL: Duration = Duration::from_secs(3);

/// The longest blocklist TTL accepted, 30 days. It also keeps a block's
/// expiration within what `SystemTime` can hold.
pub(crate) const MAX_BLOCKLIST_TTL_SEC: u64 = 30 * 24 * 60 * 60;

/// A client the node refuses until `expiration`.
struct Block {
    expiration: SystemTime,
    reason: BlockReason,
    /// Whether the block log line was written for this block. Dry run blocks
    /// without logging, so that it logs nothing for clients it still serves.
    logged: bool,
}

type BlockMap = Arc<DashMap<IpAddr, Block>>;

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

/// The clients that breached a policy, which the node refuses until their
/// blocks expire. It holds each client as [`client_prefix`] gives it, cut with
/// `ipv6_prefix_len`. A background task drops expired blocks until the
/// blocklist is dropped.
struct Blocklist {
    clients: BlockMap,
    /// Clients that reached this node through a fullnode.
    proxied_clients: BlockMap,
    ttls: BlocklistTtls,
    ipv6_prefix_len: u8,
    metrics: Arc<TrafficControllerMetrics>,
    shutdown: CancellationToken,
}

impl Drop for Blocklist {
    fn drop(&mut self) {
        self.shutdown.cancel();
    }
}

impl Blocklist {
    /// Takes the TTLs and the IPv6 prefix length from `policy_config`, and
    /// spawns the task that drops expired blocks. Panics if a TTL exceeds
    /// [`MAX_BLOCKLIST_TTL_SEC`] or the prefix length is not between
    /// [`MIN_IPV6_PREFIX_LEN`] and 128.
    fn new(policy_config: &PolicyConfig, metrics: Arc<TrafficControllerMetrics>) -> Self {
        validate_blocklist_ttls(
            Some(policy_config.connection_blocklist_ttl_sec),
            Some(policy_config.proxy_blocklist_ttl_sec),
        )
        .unwrap_or_else(|err| fatal!("Invalid traffic control config: {err}"));
        let ipv6_prefix_len = policy_config.ipv6_prefix_len;
        if !(MIN_IPV6_PREFIX_LEN..=128).contains(&ipv6_prefix_len) {
            fatal!(
                "Invalid traffic control config: the IPv6 prefix length {ipv6_prefix_len} is \
                 not between {MIN_IPV6_PREFIX_LEN} and 128"
            );
        }
        if policy_config.connection_blocklist_ttl_sec == 0 {
            warn!(
                "The connection blocklist TTL is 0, thus the node refuses no client that \
                 breaches a policy"
            );
        }
        metrics
            .connection_blocklist_ttl_sec
            .set(policy_config.connection_blocklist_ttl_sec as i64);
        metrics
            .proxy_blocklist_ttl_sec
            .set(policy_config.proxy_blocklist_ttl_sec as i64);
        let clients = BlockMap::default();
        let proxied_clients = BlockMap::default();
        let shutdown = CancellationToken::new();
        let clear_loop_clients = clients.clone();
        let clear_loop_proxied_clients = proxied_clients.clone();
        let clear_loop_metrics = metrics.clone();
        let clear_loop_shutdown = shutdown.clone();
        spawn_monitored_task!(run_clear_blocklists_loop(
            clear_loop_clients,
            clear_loop_proxied_clients,
            clear_loop_metrics,
            clear_loop_shutdown
        ));
        Self {
            clients,
            proxied_clients,
            ttls: BlocklistTtls::from_config(policy_config),
            ipv6_prefix_len,
            metrics,
            shutdown,
        }
    }

    /// Whether `client` or `proxied_client` has a block in force.
    fn blocks(&self, client: &Option<IpAddr>, proxied_client: &Option<IpAddr>) -> bool {
        let prefix = |client: &Option<IpAddr>| {
            client.map(|client| client_prefix(client, self.ipv6_prefix_len))
        };
        blocked(&prefix(client), &self.clients)
            || blocked(&prefix(proxied_client), &self.proxied_clients)
    }

    /// Refuses each client that a policy reports as breaching it. The
    /// addresses in `response` must already be cut by [`client_prefix`].
    fn block(&self, response: PolicyResponse, reason: BlockReason, dry_run: &AtomicBool) {
        if let Some(client) = response.block_client {
            self.block_client(client, reason, dry_run);
        }
        if let Some(client) = response.block_proxied_client {
            self.block_proxied_client(client, reason);
        }
    }

    /// Refuses `client` for the connection TTL, and logs the block line when
    /// this starts a block outside dry run. No-op while the TTL is zero.
    fn block_client(&self, client: IpAddr, reason: BlockReason, dry_run: &AtomicBool) {
        let ttl_secs = self.ttls.connection_blocklist_ttl_sec();
        if insert_block(
            &self.clients,
            &self.metrics.connection_ip_blocklist_len,
            client,
            ttl_secs,
            reason,
            || !dry_run.load(Ordering::Relaxed),
        ) {
            log_block(client, self.ipv6_prefix_len, reason, ttl_secs);
        }
    }

    /// Refuses `client`, which reached this node through a fullnode, for the
    /// proxy TTL. No-op while the TTL is zero. Not logged: the client never
    /// connects to this node, so a host firewall cannot act on it.
    fn block_proxied_client(&self, client: IpAddr, reason: BlockReason) {
        insert_block(
            &self.proxied_clients,
            &self.metrics.proxy_ip_blocklist_len,
            client,
            self.ttls.proxy_blocklist_ttl_sec(),
            reason,
            || false,
        );
    }

    /// Applies the TTLs that are given to the blocks applied from now on.
    fn reconfigure_ttls(
        &self,
        connection_blocklist_ttl_sec: Option<u64>,
        proxy_blocklist_ttl_sec: Option<u64>,
    ) {
        let ttls = [
            (
                connection_blocklist_ttl_sec,
                &self.ttls.connection_blocklist_ttl_sec,
                &self.metrics.connection_blocklist_ttl_sec,
            ),
            (
                proxy_blocklist_ttl_sec,
                &self.ttls.proxy_blocklist_ttl_sec,
                &self.metrics.proxy_blocklist_ttl_sec,
            ),
        ];
        for (ttl_sec, live_ttl_sec, gauge) in ttls {
            if let Some(ttl_sec) = ttl_sec {
                live_ttl_sec.store(ttl_sec, Ordering::Relaxed);
                gauge.set(ttl_sec as i64);
            }
        }
    }

    /// Logs the blocks that dry run kept from being logged, now that the node
    /// refuses those clients.
    fn log_unlogged_blocks(&self) {
        let now = SystemTime::now();
        // Logged once the shard locks are released, so that requests checked
        // meanwhile do not wait on the log.
        let mut unlogged = Vec::new();
        for mut entry in self.clients.iter_mut() {
            let client = *entry.key();
            let block = entry.value_mut();
            let Ok(remaining) = block.expiration.duration_since(now) else {
                continue;
            };
            if block.logged {
                continue;
            }
            block.logged = true;
            unlogged.push((client, block.reason, remaining.as_secs_f64().ceil() as u64));
        }
        for (client, reason, ttl_secs) in unlogged {
            log_block(client, self.ipv6_prefix_len, reason, ttl_secs);
        }
    }
}

enum Access {
    /// Every client not on the allowlist is refused, and nothing is blocked.
    /// The allowlist is populated once at initialization, and stays sorted so
    /// that `check` can binary-search it.
    Allowlist(Vec<IpAddr>),
    /// Every client on the blocklist is refused until its block expires.
    Blocklist(Blocklist),
}

/// Decides, by IP address, whether the node serves a client's request, and
/// keeps the clients it refuses.
pub(crate) struct ClientAccessManager {
    access: Access,
    metrics: Arc<TrafficControllerMetrics>,
    // Read on the request path in `check` and by the blocklist when it logs a
    // block, and toggled by the admin API.
    dry_run: AtomicBool,
}

impl ClientAccessManager {
    /// Takes the allowlist, the dry-run flag, and in blocklist mode the
    /// blocklist TTLs and the IPv6 prefix length from `policy_config`. In
    /// blocklist mode, spawns the task that drops expired blocks, so it must
    /// then be called from within a tokio runtime, and panics if a blocklist
    /// TTL exceeds [`MAX_BLOCKLIST_TTL_SEC`] or the IPv6 prefix length is not
    /// between [`MIN_IPV6_PREFIX_LEN`] and 128.
    pub(crate) fn new(
        policy_config: &PolicyConfig,
        metrics: Arc<TrafficControllerMetrics>,
    ) -> Self {
        metrics.dry_run_enabled.set(policy_config.dry_run as i64);
        let access = match &policy_config.allow_list {
            Some(allow_list) => Access::Allowlist(parse_allowlist(allow_list)),
            None => Access::Blocklist(Blocklist::new(policy_config, metrics.clone())),
        };
        Self {
            access,
            metrics,
            dry_run: AtomicBool::new(policy_config.dry_run),
        }
    }

    /// Whether to serve a request from `client`, with dry run considered. A
    /// request whose client IP the node could not resolve is refused in
    /// allowlist mode, and served in blocklist mode.
    pub(crate) fn check(&self, client: &Option<IpAddr>, proxied_client: &Option<IpAddr>) -> bool {
        let dry_run = self.dry_run();
        if client.is_none() {
            self.metrics.unresolved_client_requests.inc();
        }
        let allowed = match &self.access {
            Access::Allowlist(allowlist) => {
                client.is_some_and(|client| allowlist.binary_search(&client).is_ok())
            }
            Access::Blocklist(blocklist) => !blocklist.blocks(client, proxied_client),
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

    /// Refuses each client that a policy reports as breaching it. The
    /// addresses in `response` must already be cut by [`client_prefix`].
    /// No-op in allowlist mode.
    pub(crate) fn block(&self, response: PolicyResponse, reason: BlockReason) {
        if let Access::Blocklist(blocklist) = &self.access {
            blocklist.block(response, reason, &self.dry_run);
        }
    }

    pub(crate) fn dry_run(&self) -> bool {
        self.dry_run.load(Ordering::Relaxed)
    }

    /// The connection and proxy blocklist TTLs in force, or `None` in
    /// allowlist mode, which blocks nothing.
    pub(crate) fn blocklist_ttls_sec(&self) -> Option<(u64, u64)> {
        match &self.access {
            Access::Allowlist(_) => None,
            Access::Blocklist(blocklist) => Some((
                blocklist.ttls.connection_blocklist_ttl_sec(),
                blocklist.ttls.proxy_blocklist_ttl_sec(),
            )),
        }
    }

    /// Rejects blocklist TTLs that [`Self::reconfigure`] must not apply: any
    /// TTL in allowlist mode, and a TTL above [`MAX_BLOCKLIST_TTL_SEC`].
    pub(crate) fn validate_reconfigure(
        &self,
        connection_blocklist_ttl_sec: Option<u64>,
        proxy_blocklist_ttl_sec: Option<u64>,
    ) -> Result<(), String> {
        let sets_ttl = connection_blocklist_ttl_sec.is_some() || proxy_blocklist_ttl_sec.is_some();
        if sets_ttl && matches!(self.access, Access::Allowlist(_)) {
            return Err("Cannot reconfigure the blocklist TTLs in allowlist mode".to_string());
        }
        validate_blocklist_ttls(connection_blocklist_ttl_sec, proxy_blocklist_ttl_sec)
    }

    /// Applies the settings that are given, and leaves the others as they are.
    /// A changed TTL applies to the blocks applied from now on. The TTLs must
    /// pass [`Self::validate_reconfigure`].
    pub(crate) fn reconfigure(
        &self,
        dry_run: Option<bool>,
        connection_blocklist_ttl_sec: Option<u64>,
        proxy_blocklist_ttl_sec: Option<u64>,
    ) {
        if let Some(dry_run) = dry_run {
            self.metrics.dry_run_enabled.set(dry_run as i64);
            let was_dry_run = self.dry_run.swap(dry_run, Ordering::Relaxed);
            if let (true, false, Access::Blocklist(blocklist)) =
                (was_dry_run, dry_run, &self.access)
            {
                blocklist.log_unlogged_blocks();
            }
        }
        if let Access::Blocklist(blocklist) = &self.access {
            blocklist.reconfigure_ttls(connection_blocklist_ttl_sec, proxy_blocklist_ttl_sec);
        }
    }
}

/// Rejects a blocklist TTL above [`MAX_BLOCKLIST_TTL_SEC`].
fn validate_blocklist_ttls(
    connection_blocklist_ttl_sec: Option<u64>,
    proxy_blocklist_ttl_sec: Option<u64>,
) -> Result<(), String> {
    for (ttl_sec, name) in [
        (connection_blocklist_ttl_sec, "connection"),
        (proxy_blocklist_ttl_sec, "proxy"),
    ] {
        if let Some(ttl_sec) = ttl_sec.filter(|ttl_sec| *ttl_sec > MAX_BLOCKLIST_TTL_SEC) {
            return Err(format!(
                "The {name} blocklist TTL {ttl_sec} exceeds the maximum of \
                 {MAX_BLOCKLIST_TTL_SEC} seconds"
            ));
        }
    }
    Ok(())
}

/// The client that the policies count and the blocklists hold. An IPv4
/// client is its full address. An IPv6 client is the first `ipv6_prefix_len`
/// bits of its address, with the rest set to zero: the node cannot see how
/// long a client's own network prefix is, so it assumes the same length for
/// every IPv6 client. An IPv4 client seen as an IPv4-mapped IPv6 address
/// counts as that IPv4 address. See [`ipv6_prefix_len_of`] for the IPv6
/// addresses kept whole.
pub(crate) fn client_prefix(client: IpAddr, ipv6_prefix_len: u8) -> IpAddr {
    match client.to_canonical() {
        IpAddr::V6(address) => {
            let prefix_len = ipv6_prefix_len_of(address, ipv6_prefix_len);
            let mask = u128::MAX
                .checked_shl(128 - u32::from(prefix_len.min(128)))
                .unwrap_or(0);
            IpAddr::V6(Ipv6Addr::from(u128::from(address) & mask))
        }
        IpAddr::V4(address) => IpAddr::V4(address),
    }
}

/// The shortest IPv6 prefix length accepted. The ranges that
/// [`ipv6_prefix_len_of`] keeps whole are set by at most the first 32 bits, so
/// that cutting any other address to 32 bits or more never lands it in one.
pub(crate) const MIN_IPV6_PREFIX_LEN: u8 = 32;

/// How many leading bits of `address` identify its client: `ipv6_prefix_len`,
/// or all 128 where unrelated clients share the leading bits. These are
/// `::/32`, with the unspecified, loopback and IPv4-compatible addresses,
/// link-local addresses (`fe80::/10`), Teredo addresses (`2001::/32`), and
/// the NAT64 prefixes in `64:ff9b::/32`, each of whose addresses carries one
/// IPv4 client.
pub(crate) fn ipv6_prefix_len_of(address: Ipv6Addr, ipv6_prefix_len: u8) -> u8 {
    let segments = address.segments();
    let kept_whole = segments[..2] == [0, 0]
        || segments[0] & 0xffc0 == 0xfe80
        || segments[..2] == [0x2001, 0]
        || segments[..2] == [0x64, 0xff9b];
    if kept_whole { 128 } else { ipv6_prefix_len }
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

fn blocked(client: &Option<IpAddr>, blocklist: &BlockMap) -> bool {
    client.is_some_and(|client| {
        blocklist
            .get(&client)
            .is_some_and(|block| SystemTime::now() < block.expiration)
    })
}

/// Blocks a client for `ttl_secs`, counting it only when it was not already
/// blocked so that the gauge matches the blocklist length. A TTL of zero
/// blocks nothing. Returns whether to log the block: when `log` returns true,
/// once per block rather than once per breach, and again once the block has
/// expired.
fn insert_block(
    blocklist: &BlockMap,
    len_gauge: &IntGauge,
    client: IpAddr,
    ttl_secs: u64,
    reason: BlockReason,
    log: impl FnOnce() -> bool,
) -> bool {
    if ttl_secs == 0 {
        return false;
    }
    let now = SystemTime::now();
    let expiration = now + Duration::from_secs(ttl_secs);
    let entry = blocklist.entry(client);
    // Asked under the entry lock, which `log_unlogged_blocks` also takes, so
    // that a block applied while dry run is turned off is logged either here
    // or there.
    let log = log();
    match entry {
        Entry::Vacant(entry) => {
            debug!("Adding client {client:?} to blocklist");
            len_gauge.inc();
            entry.insert(Block {
                expiration,
                reason,
                logged: log,
            });
            log
        }
        Entry::Occupied(mut entry) => {
            let block = entry.get_mut();
            let active = now < block.expiration;
            let already_logged = block.logged && active;
            // A breach never shortens an active block, so that the block lasts
            // at least as long as its log line told the firewall.
            let expiration = if active {
                block.expiration.max(expiration)
            } else {
                expiration
            };
            *block = Block {
                expiration,
                reason,
                logged: already_logged || log,
            };
            log && !already_logged
        }
    }
}

/// Waits for the next tick of a background loop, returning false once the
/// blocklist holding the loop's state has been dropped.
async fn tick(interval: &mut Interval, shutdown: &CancellationToken) -> bool {
    tokio::select! {
        _ = interval.tick() => true,
        _ = shutdown.cancelled() => false,
    }
}

/// Drops expired blocklist entries and refreshes the length gauges.
async fn run_clear_blocklists_loop(
    clients: BlockMap,
    proxied_clients: BlockMap,
    metrics: Arc<TrafficControllerMetrics>,
    shutdown: CancellationToken,
) {
    let mut interval = tokio::time::interval(CLEAR_BLOCKLIST_INTERVAL);
    // A slow pass delays the next one rather than causing a burst of passes.
    interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
    while tick(&mut interval, &shutdown).await {
        let now = SystemTime::now();
        clients.retain(|_, block| now < block.expiration);
        proxied_clients.retain(|_, block| now < block.expiration);
        metrics
            .connection_ip_blocklist_len
            .set(clients.len() as i64);
        metrics
            .proxy_ip_blocklist_len
            .set(proxied_clients.len() as i64);
    }
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use super::*;

    const CLIENT: IpAddr = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));

    impl ClientAccessManager {
        fn blocklist(&self) -> Option<&Blocklist> {
            match &self.access {
                Access::Blocklist(blocklist) => Some(blocklist),
                Access::Allowlist(_) => None,
            }
        }

        /// Blocks the direct client `client`, as a policy response does.
        fn block_client(&self, client: IpAddr, reason: BlockReason) {
            let response = PolicyResponse {
                block_client: Some(client),
                ..Default::default()
            };
            self.block(response, reason);
        }

        /// Blocks `client`, which reached this node through a fullnode, as a
        /// policy response does.
        fn block_proxied_client(&self, client: IpAddr, reason: BlockReason) {
            let response = PolicyResponse {
                block_proxied_client: Some(client),
                ..Default::default()
            };
            self.block(response, reason);
        }

        /// Moves the expiration of every direct client's block into the past,
        /// as if its TTL had passed.
        pub(crate) fn expire_client_blocks(&self) {
            if let Some(blocklist) = self.blocklist() {
                let past = SystemTime::now() - Duration::from_secs(1);
                for mut block in blocklist.clients.iter_mut() {
                    block.expiration = past;
                }
            }
        }

        /// When the block of the direct client `client` expires, if it has one.
        fn client_block_expiration(&self, client: IpAddr) -> Option<SystemTime> {
            self.blocklist()?
                .clients
                .get(&client)
                .map(|block| block.expiration)
        }
    }

    fn manager(policy_config: PolicyConfig) -> ClientAccessManager {
        ClientAccessManager::new(
            &policy_config,
            Arc::new(TrafficControllerMetrics::new_for_tests()),
        )
    }

    fn blocklist_mode(dry_run: bool, ttl_sec: u64) -> PolicyConfig {
        PolicyConfig {
            connection_blocklist_ttl_sec: ttl_sec,
            proxy_blocklist_ttl_sec: ttl_sec,
            dry_run,
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn a_blocked_client_is_refused_without_any_policy() {
        let manager = manager(blocklist_mode(false, 120));
        assert!(manager.check(&Some(CLIENT), &None));

        manager.block_client(CLIENT, BlockReason::Spam);
        assert!(!manager.check(&Some(CLIENT), &None));

        manager.block_proxied_client(CLIENT, BlockReason::Spam);
        assert!(!manager.check(&None, &Some(CLIENT)));
    }

    #[tokio::test]
    async fn a_block_ends_with_its_ttl() {
        let manager = manager(blocklist_mode(false, 120));
        manager.block_client(CLIENT, BlockReason::Spam);
        manager.expire_client_blocks();
        assert!(manager.check(&Some(CLIENT), &None));
    }

    #[tokio::test]
    async fn a_breach_never_shortens_an_active_block() {
        let manager = manager(blocklist_mode(false, 3600));
        manager.block_client(CLIENT, BlockReason::Spam);
        let first_expiration = manager.client_block_expiration(CLIENT).unwrap();

        manager.reconfigure(None, Some(60), None);
        manager.block_client(CLIENT, BlockReason::Spam);
        assert_eq!(
            manager.client_block_expiration(CLIENT),
            Some(first_expiration)
        );
    }

    #[tokio::test]
    async fn a_zero_ttl_blocks_nothing() {
        let manager = manager(blocklist_mode(false, 0));
        manager.block_client(CLIENT, BlockReason::Spam);
        manager.block_proxied_client(CLIENT, BlockReason::Spam);
        assert!(manager.check(&Some(CLIENT), &Some(CLIENT)));
        assert_eq!(manager.metrics.connection_ip_blocklist_len.get(), 0);
        assert_eq!(manager.metrics.proxy_ip_blocklist_len.get(), 0);
    }

    #[tokio::test]
    async fn the_maximum_ttl_blocks() {
        let manager = manager(blocklist_mode(false, MAX_BLOCKLIST_TTL_SEC));
        manager.block_client(CLIENT, BlockReason::Spam);
        manager.block_proxied_client(CLIENT, BlockReason::Spam);
        assert!(!manager.check(&Some(CLIENT), &None));
        assert!(!manager.check(&None, &Some(CLIENT)));
    }

    #[tokio::test]
    #[should_panic(expected = "The connection blocklist TTL 2592001 exceeds the maximum")]
    async fn a_connection_blocklist_ttl_above_the_maximum_is_rejected() {
        manager(PolicyConfig {
            connection_blocklist_ttl_sec: MAX_BLOCKLIST_TTL_SEC + 1,
            ..blocklist_mode(false, 120)
        });
    }

    #[tokio::test]
    #[should_panic(expected = "The proxy blocklist TTL 2592001 exceeds the maximum")]
    async fn a_proxy_blocklist_ttl_above_the_maximum_is_rejected() {
        manager(PolicyConfig {
            proxy_blocklist_ttl_sec: MAX_BLOCKLIST_TTL_SEC + 1,
            ..blocklist_mode(false, 120)
        });
    }

    #[tokio::test]
    async fn dry_run_serves_a_blocked_client_and_counts_it() {
        let manager = manager(blocklist_mode(true, 120));
        manager.block_client(CLIENT, BlockReason::Spam);
        assert!(manager.check(&Some(CLIENT), &None));
        assert_eq!(manager.metrics.num_dry_run_blocked_requests.get(), 1);

        manager.reconfigure(Some(false), None, None);
        assert!(!manager.check(&Some(CLIENT), &None));
    }

    #[tokio::test]
    async fn dropping_the_blocklist_stops_its_cleanup_task() {
        let manager = manager(blocklist_mode(false, 120));
        let shutdown = manager.blocklist().unwrap().shutdown.clone();
        assert!(!shutdown.is_cancelled());
        drop(manager);
        assert!(shutdown.is_cancelled());
    }

    #[test]
    fn allowlist_mode_sets_no_blocklist_ttl_gauges() {
        let manager = manager(PolicyConfig {
            allow_list: Some(vec![CLIENT.to_string()]),
            connection_blocklist_ttl_sec: 120,
            proxy_blocklist_ttl_sec: 120,
            ..Default::default()
        });
        assert_eq!(manager.metrics.connection_blocklist_ttl_sec.get(), 0);
        assert_eq!(manager.metrics.proxy_blocklist_ttl_sec.get(), 0);
    }

    #[test]
    fn allowlist_mode_blocks_nothing() {
        let manager = manager(PolicyConfig {
            allow_list: Some(vec![CLIENT.to_string()]),
            dry_run: false,
            ..Default::default()
        });
        manager.block_client(CLIENT, BlockReason::Spam);
        assert!(manager.check(&Some(CLIENT), &None));
    }

    fn v6(segments: [u16; 8]) -> IpAddr {
        IpAddr::V6(Ipv6Addr::from(segments))
    }

    #[test]
    fn an_ipv6_client_is_its_address_prefix() {
        let client = v6([0x2001, 0xdb8, 1, 2, 3, 4, 5, 6]);
        assert_eq!(
            client_prefix(client, 64),
            v6([0x2001, 0xdb8, 1, 2, 0, 0, 0, 0])
        );
        assert_eq!(
            client_prefix(client, 48),
            v6([0x2001, 0xdb8, 1, 0, 0, 0, 0, 0])
        );
        assert_eq!(client_prefix(client, 128), client);
    }

    #[test]
    fn an_ipv4_client_is_its_full_address() {
        assert_eq!(client_prefix(CLIENT, 64), CLIENT);
        let IpAddr::V4(address) = CLIENT else {
            unreachable!()
        };
        let mapped = IpAddr::V6(address.to_ipv6_mapped());
        assert_eq!(client_prefix(mapped, 64), CLIENT);
    }

    #[test]
    fn ipv6_addresses_whose_leading_bits_are_shared_are_kept_whole() {
        for client in [
            v6([0, 0, 0, 0, 0, 0, 0, 1]),
            v6([0, 0, 0, 0, 0, 0, 0x0a00, 0x0001]),
            v6([0xfe80, 0, 0, 0, 1, 2, 3, 4]),
            v6([0x2001, 0, 0x4136, 0xe378, 0x8000, 0x63bf, 0x3fff, 0xfdd2]),
            v6([0x64, 0xff9b, 0, 0, 0, 0, 0x0a00, 0x0001]),
            v6([0x64, 0xff9b, 1, 0x0a00, 0x0001, 0, 0, 0]),
        ] {
            assert_eq!(client_prefix(client, 64), client);
        }
    }

    #[test]
    fn a_cut_address_keeps_the_prefix_length_of_its_address() {
        // The block log line works the prefix length out again from the
        // blocklist key, so cutting must not move an address into or out of
        // a range kept whole.
        for client in [
            v6([0x2001, 0xdb8, 1, 2, 3, 4, 5, 6]),
            v6([0x2001, 1, 2, 3, 4, 5, 6, 7]),
            v6([0xfec0, 0, 0, 0, 0, 0, 0, 1]),
            v6([0x64, 0xff9a, 1, 2, 3, 4, 5, 6]),
            v6([0, 1, 2, 3, 4, 5, 6, 7]),
            v6([0, 0, 0, 0, 0xffff, 0, 0x0a00, 1]),
            v6([0xfe80, 0, 0, 0, 1, 2, 3, 4]),
        ] {
            let IpAddr::V6(address) = client else {
                unreachable!()
            };
            for ipv6_prefix_len in MIN_IPV6_PREFIX_LEN..=128 {
                let IpAddr::V6(key) = client_prefix(client, ipv6_prefix_len) else {
                    unreachable!()
                };
                assert_eq!(
                    ipv6_prefix_len_of(key, ipv6_prefix_len),
                    ipv6_prefix_len_of(address, ipv6_prefix_len),
                    "{client} at /{ipv6_prefix_len}"
                );
            }
        }
    }

    #[tokio::test]
    async fn a_block_covers_the_ipv6_prefix_of_the_client() {
        let manager = manager(blocklist_mode(false, 120));
        manager.block_client(
            client_prefix(v6([0x2001, 0xdb8, 1, 2, 0, 0, 0, 1]), 64),
            BlockReason::Spam,
        );
        assert!(!manager.check(&Some(v6([0x2001, 0xdb8, 1, 2, 0, 0, 0, 0xffff])), &None));
        assert!(manager.check(&Some(v6([0x2001, 0xdb8, 1, 3, 0, 0, 0, 1])), &None));
    }

    #[tokio::test]
    async fn a_prefix_length_of_128_blocks_only_the_address() {
        let manager = manager(PolicyConfig {
            ipv6_prefix_len: 128,
            ..blocklist_mode(false, 120)
        });
        let client = v6([0x2001, 0xdb8, 1, 2, 0, 0, 0, 1]);
        manager.block_client(client, BlockReason::Spam);
        assert!(!manager.check(&Some(client), &None));
        assert!(manager.check(&Some(v6([0x2001, 0xdb8, 1, 2, 0, 0, 0, 2])), &None));
    }

    #[tokio::test]
    #[should_panic(expected = "the IPv6 prefix length 31 is not between 32 and 128")]
    async fn a_prefix_length_below_32_is_rejected() {
        manager(PolicyConfig {
            ipv6_prefix_len: 31,
            ..blocklist_mode(false, 120)
        });
    }

    #[tokio::test]
    #[should_panic(expected = "the IPv6 prefix length 129 is not between 32 and 128")]
    async fn a_prefix_length_above_128_is_rejected() {
        manager(PolicyConfig {
            ipv6_prefix_len: 129,
            ..blocklist_mode(false, 120)
        });
    }

    #[test]
    fn the_allowlist_holds_single_ipv6_addresses() {
        let allowed = v6([0x2001, 0xdb8, 1, 2, 0, 0, 0, 1]);
        let manager = manager(PolicyConfig {
            allow_list: Some(vec![allowed.to_string()]),
            dry_run: false,
            ..Default::default()
        });
        assert!(manager.check(&Some(allowed), &None));
        assert!(!manager.check(&Some(v6([0x2001, 0xdb8, 1, 2, 0, 0, 0, 2])), &None));
    }
}
