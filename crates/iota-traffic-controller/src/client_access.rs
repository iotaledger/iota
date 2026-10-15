// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Decides, by IP address, whether the node serves a client's request: either
//! every client not on the allowlist is refused, or every client on the
//! blocklist is, until its block expires.

use std::{
    net::IpAddr,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, SystemTime},
};

use dashmap::DashMap;
use iota_common::fatal;
use iota_metrics::spawn_monitored_task;
use iota_types::traffic_control::PolicyConfig;
use prometheus_filtered::IntGauge;
use tokio_util::sync::CancellationToken;
use tracing::debug;

use crate::{metrics::TrafficControllerMetrics, parse_ip};

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

enum Access {
    /// Every client not on the allowlist is refused, and nothing is blocked.
    /// The allowlist is populated once at initialization, and stays sorted so
    /// that `check` can binary-search it.
    Allowlist(Vec<IpAddr>),
    /// Every client on the blocklists is refused until its block expires. The
    /// background task that drops expired blocks stops on `shutdown`.
    Blocklist {
        blocklists: Blocklists,
        shutdown: CancellationToken,
    },
}

/// Decides, by IP address, whether the node serves a client's request, and
/// keeps the clients it refuses.
pub struct ClientAccessManager {
    access: Access,
    metrics: Arc<TrafficControllerMetrics>,
    // Read on the request path in `check` and toggled by the admin API.
    dry_run: AtomicBool,
    // Read whenever a block is applied and changed by the admin API.
    blocklist_ttls: BlocklistTtls,
}

impl Drop for ClientAccessManager {
    fn drop(&mut self) {
        if let Access::Blocklist { shutdown, .. } = &self.access {
            shutdown.cancel();
        }
    }
}

impl ClientAccessManager {
    /// Takes the allowlist, the blocklist TTLs and the dry-run flag from
    /// `policy_config`. Without an allowlist, spawns the task that drops
    /// expired blocks, so it must then be called from within a tokio runtime.
    pub fn new(policy_config: &PolicyConfig, metrics: Arc<TrafficControllerMetrics>) -> Self {
        metrics.dry_run_enabled.set(policy_config.dry_run as i64);
        metrics
            .connection_blocklist_ttl_sec
            .set(policy_config.connection_blocklist_ttl_sec as i64);
        metrics
            .proxy_blocklist_ttl_sec
            .set(policy_config.proxy_blocklist_ttl_sec as i64);
        let access = match &policy_config.allow_list {
            Some(allow_list) => Access::Allowlist(parse_allowlist(allow_list)),
            None => {
                let blocklists = Blocklists {
                    clients: Arc::new(DashMap::new()),
                    proxied_clients: Arc::new(DashMap::new()),
                };
                let shutdown = CancellationToken::new();
                let clear_loop_blocklists = blocklists.clone();
                let clear_loop_metrics = metrics.clone();
                let clear_loop_shutdown = shutdown.clone();
                spawn_monitored_task!(run_clear_blocklists_loop(
                    clear_loop_blocklists,
                    clear_loop_metrics,
                    clear_loop_shutdown
                ));
                Access::Blocklist {
                    blocklists,
                    shutdown,
                }
            }
        };
        Self {
            access,
            metrics,
            dry_run: AtomicBool::new(policy_config.dry_run),
            blocklist_ttls: BlocklistTtls::from_config(policy_config),
        }
    }

    /// Whether to serve a request from `client`, with dry run considered. A
    /// request whose client IP the node could not resolve is refused in
    /// allowlist mode, and served in blocklist mode.
    pub fn check(&self, client: &Option<IpAddr>, proxied_client: &Option<IpAddr>) -> bool {
        let dry_run = self.dry_run();
        if client.is_none() {
            self.metrics.unresolved_client_requests.inc();
        }
        let allowed = match &self.access {
            Access::Allowlist(allowlist) => {
                client.is_some_and(|client| allowlist.binary_search(&client).is_ok())
            }
            Access::Blocklist { blocklists, .. } => {
                !blocked(client, &blocklists.clients)
                    && !blocked(proxied_client, &blocklists.proxied_clients)
            }
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

    /// Refuses `client` for the connection blocklist TTL. No-op in allowlist
    /// mode.
    pub fn block_client(&self, client: IpAddr) {
        if let Access::Blocklist { blocklists, .. } = &self.access {
            insert_block(
                &blocklists.clients,
                &self.metrics.connection_ip_blocklist_len,
                client,
                self.blocklist_ttls.connection_blocklist_ttl_sec(),
            );
        }
    }

    /// Refuses `client`, which reached this node through a fullnode, for the
    /// proxy blocklist TTL. No-op in allowlist mode.
    pub fn block_proxied_client(&self, client: IpAddr) {
        if let Access::Blocklist { blocklists, .. } = &self.access {
            insert_block(
                &blocklists.proxied_clients,
                &self.metrics.proxy_ip_blocklist_len,
                client,
                self.blocklist_ttls.proxy_blocklist_ttl_sec(),
            );
        }
    }

    pub fn dry_run(&self) -> bool {
        self.dry_run.load(Ordering::Relaxed)
    }

    pub fn connection_blocklist_ttl_sec(&self) -> u64 {
        self.blocklist_ttls.connection_blocklist_ttl_sec()
    }

    pub fn proxy_blocklist_ttl_sec(&self) -> u64 {
        self.blocklist_ttls.proxy_blocklist_ttl_sec()
    }

    /// Applies the settings that are given, and leaves the others as they are.
    /// A changed TTL applies to the blocks applied from now on.
    pub fn reconfigure(
        &self,
        dry_run: Option<bool>,
        connection_blocklist_ttl_sec: Option<u64>,
        proxy_blocklist_ttl_sec: Option<u64>,
    ) {
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

fn blocked(client: &Option<IpAddr>, blocklist: &Blocklist) -> bool {
    client.is_some_and(|client| {
        blocklist
            .get(&client)
            .is_some_and(|expiration| SystemTime::now() < *expiration)
    })
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

/// Waits for the next tick of a background loop, returning false once the
/// manager holding the loop's state has been dropped.
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

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use super::*;

    const CLIENT: IpAddr = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));

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

        manager.block_client(CLIENT);
        assert!(!manager.check(&Some(CLIENT), &None));

        manager.block_proxied_client(CLIENT);
        assert!(!manager.check(&None, &Some(CLIENT)));
    }

    #[tokio::test]
    async fn a_block_ends_with_its_ttl() {
        // A TTL of zero expires the block at once.
        let manager = manager(blocklist_mode(false, 0));
        manager.block_client(CLIENT);
        assert!(manager.check(&Some(CLIENT), &None));
    }

    #[tokio::test]
    async fn dry_run_serves_a_blocked_client_and_counts_it() {
        let manager = manager(blocklist_mode(true, 120));
        manager.block_client(CLIENT);
        assert!(manager.check(&Some(CLIENT), &None));
        assert_eq!(manager.metrics.num_dry_run_blocked_requests.get(), 1);

        manager.reconfigure(Some(false), None, None);
        assert!(!manager.check(&Some(CLIENT), &None));
    }

    #[test]
    fn allowlist_mode_blocks_nothing() {
        let manager = manager(PolicyConfig {
            allow_list: Some(vec![CLIENT.to_string()]),
            dry_run: false,
            ..Default::default()
        });
        manager.block_client(CLIENT);
        assert!(manager.check(&Some(CLIENT), &None));
    }
}
