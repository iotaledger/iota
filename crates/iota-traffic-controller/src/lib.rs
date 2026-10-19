// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Protocol level traffic control. Tallies are charged to the spam and error
//! policies inline, so a breaching client is blocked before
//! [`TrafficController::tally`] returns.

mod block_log;
mod client_access;
pub mod metrics;
pub mod policies;
pub mod sim;

use std::{
    fmt::Debug,
    net::{IpAddr, SocketAddr},
    str,
    sync::Arc,
};

use iota_types::{
    error::IotaError,
    traffic_control::{ClientIdSource, PolicyConfig, PolicyType, TrafficControlReconfigParams},
};
use tracing::error;

use self::{
    block_log::BlockReason,
    client_access::{ClientAccessManager, client_prefix},
    metrics::TrafficControllerMetrics,
};
use crate::policies::{MAX_CLIENT_THRESHOLD, TrafficControlPolicy, TrafficTally};

/// The spam and error policies. Absent in allowlist mode, which does no
/// tallying.
struct Policies {
    spam: TrafficControlPolicy,
    error: TrafficControlPolicy,
}

#[derive(Clone)]
pub struct TrafficController {
    client_access: Arc<ClientAccessManager>,
    policies: Option<Arc<Policies>>,
    policy_config: Arc<PolicyConfig>,
    metrics: Arc<TrafficControllerMetrics>,
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
    /// Must be called from within a tokio runtime unless `policy_config` has
    /// an allowlist, since blocklist mode spawns the task that drops expired
    /// blocks.
    pub fn init(policy_config: PolicyConfig, metrics: Arc<TrafficControllerMetrics>) -> Self {
        let client_access = Arc::new(ClientAccessManager::new(&policy_config, metrics.clone()));
        let policies = policy_config.allow_list.is_none().then(|| {
            let policies = Policies {
                spam: TrafficControlPolicy::from_policy_type(
                    &policy_config.spam_policy_type,
                    policy_config.connection_blocklist_ttl_sec,
                    metrics.rate_limiter_evictions.clone(),
                ),
                error: TrafficControlPolicy::from_policy_type(
                    &policy_config.error_policy_type,
                    policy_config.connection_blocklist_ttl_sec,
                    metrics.rate_limiter_evictions.clone(),
                ),
            };
            set_policy_config_metrics(&policies, &policy_config, &metrics);
            Arc::new(policies)
        });
        Self {
            client_access,
            policies,
            policy_config: Arc::new(policy_config),
            metrics,
        }
    }

    pub fn init_for_test(policy_config: PolicyConfig) -> Self {
        Self::init(
            policy_config,
            Arc::new(TrafficControllerMetrics::new_for_tests()),
        )
    }

    /// The settings in force. The thresholds and the blocklist TTLs are
    /// `None` in allowlist mode, which neither tallies nor blocks.
    pub fn get_current_state(&self) -> TrafficControlReconfigParams {
        let ttls = self.client_access.blocklist_ttls_sec();
        TrafficControlReconfigParams {
            error_threshold: self
                .policies
                .as_ref()
                .and_then(|policies| policies.error.client_threshold()),
            spam_threshold: self
                .policies
                .as_ref()
                .and_then(|policies| policies.spam.client_threshold()),
            dry_run: Some(self.client_access.dry_run()),
            connection_blocklist_ttl_sec: ttls.map(|(connection, _)| connection),
            proxy_blocklist_ttl_sec: ttls.map(|(_, proxy)| proxy),
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
                self.policies.as_ref().map(|policies| &policies.error),
                &self.metrics.error_client_threshold,
                "error",
            ),
            (
                spam_threshold,
                self.policies.as_ref().map(|policies| &policies.spam),
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
        self.client_access
            .validate_reconfigure(connection_blocklist_ttl_sec, proxy_blocklist_ttl_sec)
            .map_err(IotaError::InvalidAdminRequest)?;
        for (threshold, policy, gauge, _) in updates {
            if let (Some(threshold), Some(policy)) = (threshold, policy) {
                policy.set_client_threshold(threshold);
                gauge.set(threshold as i64);
            }
        }
        self.client_access.reconfigure(
            dry_run,
            connection_blocklist_ttl_sec,
            proxy_blocklist_ttl_sec,
        );

        Ok(self.get_current_state())
    }

    /// Charges the tally against the spam and error policies, and has the
    /// client access manager refuse each client that breaches one, before
    /// returning. An IPv6 client is charged for the first `ipv6-prefix-len`
    /// bits of its address. No-op in allowlist mode.
    pub fn tally(&self, mut tally: TrafficTally) {
        let Some(policies) = &self.policies else {
            return;
        };
        let ipv6_prefix_len = self.policy_config.ipv6_prefix_len;
        tally.direct = tally
            .direct
            .map(|client| client_prefix(client, ipv6_prefix_len));
        tally.through_fullnode = tally
            .through_fullnode
            .map(|client| client_prefix(client, ipv6_prefix_len));
        self.metrics.tallies.inc();
        if tally.spam_weight.is_sampled() && self.policy_config.spam_sample_rate.is_sampled() {
            let response = policies.spam.charge(&tally);
            self.metrics.tally_handled.inc();
            self.client_access.block(response, BlockReason::Spam);
        }
        if let Some((error_weight, error_type)) = &tally.error_info {
            if error_weight.is_sampled() {
                self.metrics
                    .tally_error_types
                    .with_label_values(&[error_type.as_str()])
                    .inc();
                let response = policies.error.charge(&tally);
                self.metrics.error_tally_handled.inc();
                self.client_access.block(response, BlockReason::Error);
            }
        }
    }

    /// Whether to serve a request from `client`, with dry run considered. A
    /// request whose client IP the node could not resolve is refused in
    /// allowlist mode, and served in blocklist mode.
    pub fn check(&self, client: &Option<IpAddr>, proxied_client: &Option<IpAddr>) -> bool {
        self.client_access.check(client, proxied_client)
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
    policies: &Policies,
    policy_config: &PolicyConfig,
    metrics: &TrafficControllerMetrics,
) {
    if let Some(threshold) = policies.spam.client_threshold() {
        metrics.spam_client_threshold.set(threshold as i64);
    }
    if let Some(threshold) = policies.error.client_threshold() {
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
    use crate::{block_log::tests::capture_plain, client_access::MAX_BLOCKLIST_TTL_SEC};

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
            // A TTL of zero would block nothing.
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
    async fn a_block_logs_one_line_however_often_the_client_breaches() {
        let controller = TrafficController::init_for_test(policy_config(false, PolicyKind::Spam));
        let log = capture_plain(|| {
            for _ in 0..3 {
                controller.tally(breach(PolicyKind::Spam));
            }
        });
        let [line] = log.block_lines().try_into().unwrap();
        assert!(
            line.ends_with("client_blocked client=10.0.0.1 reason=spam ttl_secs=120"),
            "{line}"
        );
    }

    #[tokio::test]
    async fn an_error_block_logs_the_error_reason() {
        let controller = TrafficController::init_for_test(policy_config(false, PolicyKind::Error));
        let log = capture_plain(|| controller.tally(breach(PolicyKind::Error)));
        let [line] = log.block_lines().try_into().unwrap();
        assert!(
            line.ends_with("client_blocked client=10.0.0.1 reason=error ttl_secs=120"),
            "{line}"
        );
    }

    #[tokio::test]
    async fn a_breach_after_the_block_expired_logs_again() {
        let controller = TrafficController::init_for_test(policy_config(false, PolicyKind::Spam));
        let log = capture_plain(|| {
            controller.tally(breach(PolicyKind::Spam));
            controller.client_access.expire_client_blocks();
            controller.tally(breach(PolicyKind::Spam));
        });
        assert_eq!(log.block_lines().len(), 2);
    }

    #[tokio::test]
    async fn dry_run_logs_no_block() {
        let controller = TrafficController::init_for_test(policy_config(true, PolicyKind::Spam));
        let log = capture_plain(|| controller.tally(breach(PolicyKind::Spam)));
        assert_eq!(log.block_lines(), Vec::<String>::new());
    }

    #[tokio::test]
    async fn turning_dry_run_off_logs_the_blocks_that_take_effect() {
        let controller = TrafficController::init_for_test(policy_config(true, PolicyKind::Spam));
        controller.tally(breach(PolicyKind::Spam));

        // The node refuses the client from now on, and does not tally the
        // requests it refuses, so the line has to come from the switch.
        let log = capture_plain(|| {
            controller
                .admin_reconfigure(TrafficControlReconfigParams {
                    error_threshold: None,
                    spam_threshold: None,
                    dry_run: Some(false),
                    connection_blocklist_ttl_sec: None,
                    proxy_blocklist_ttl_sec: None,
                })
                .expect("the request changes only the dry-run flag");
        });
        let [line] = log.block_lines().try_into().unwrap();
        assert!(
            line.ends_with("client_blocked client=10.0.0.1 reason=spam ttl_secs=120"),
            "{line}"
        );
    }

    #[tokio::test]
    async fn the_addresses_of_one_ipv6_prefix_are_counted_and_blocked_together() {
        let controller = TrafficController::init_for_test(PolicyConfig {
            spam_policy_type: PolicyType::TestNConnIP(2),
            ..policy_config(false, PolicyKind::Spam)
        });
        let address =
            |last| IpAddr::V6(std::net::Ipv6Addr::new(0x2001, 0xdb8, 1, 2, 0, 0, 0, last));
        let tally = |client| TrafficTally::new(Some(client), None, None, Weight::one());

        // Neither address reaches the threshold of two alone.
        let log = capture_plain(|| {
            controller.tally(tally(address(1)));
            controller.tally(tally(address(2)));
        });
        let [line] = log.block_lines().try_into().unwrap();
        assert!(
            line.ends_with("client_blocked client=2001:db8:1:2::/64 reason=spam ttl_secs=120"),
            "{line}"
        );
        assert!(!controller.check(&Some(address(3)), &None));
    }

    #[tokio::test]
    async fn a_block_of_a_client_behind_a_fullnode_is_not_logged() {
        let controller = TrafficController::init_for_test(PolicyConfig {
            spam_policy_type: freq_threshold(0),
            spam_sample_rate: Weight::one(),
            dry_run: false,
            ..Default::default()
        });
        let log = capture_plain(|| {
            controller.tally(TrafficTally::new(None, Some(CLIENT), None, Weight::one()))
        });
        assert!(!controller.check(&None, &Some(CLIENT)));
        assert_eq!(log.block_lines(), Vec::<String>::new());
    }

    #[tokio::test]
    async fn test_the_admin_api_changes_the_blocklist_ttl() {
        let controller = TrafficController::init_for_test(PolicyConfig {
            connection_blocklist_ttl_sec: 60,
            ..policy_config(false, PolicyKind::Spam)
        });

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
        let log = capture_plain(|| controller.tally(breach(PolicyKind::Spam)));
        let [line] = log.block_lines().try_into().unwrap();
        assert!(line.ends_with(" ttl_secs=120"), "{line}");
        assert!(!controller.check(&Some(CLIENT), &None));
        assert_eq!(controller.metrics.connection_blocklist_ttl_sec.get(), 120);
    }

    #[tokio::test]
    async fn test_the_admin_api_rejects_a_blocklist_ttl_above_the_maximum() {
        let controller = TrafficController::init_for_test(policy_config(false, PolicyKind::Spam));
        for (connection_blocklist_ttl_sec, proxy_blocklist_ttl_sec) in [
            (Some(MAX_BLOCKLIST_TTL_SEC + 1), Some(5)),
            (Some(5), Some(MAX_BLOCKLIST_TTL_SEC + 1)),
        ] {
            let result = controller.admin_reconfigure(TrafficControlReconfigParams {
                error_threshold: None,
                spam_threshold: None,
                dry_run: Some(true),
                connection_blocklist_ttl_sec,
                proxy_blocklist_ttl_sec,
            });
            assert!(matches!(result, Err(IotaError::InvalidAdminRequest(_))));
        }

        // A rejected request applies nothing, not even its valid settings.
        let state = controller.get_current_state();
        assert_eq!(state.dry_run, Some(false));
        assert_eq!(state.connection_blocklist_ttl_sec, Some(120));
        assert_eq!(
            state.proxy_blocklist_ttl_sec,
            Some(default_blocklist_ttl_sec())
        );
    }

    #[tokio::test]
    async fn test_a_zero_ttl_from_the_admin_api_turns_blocking_off() {
        let controller = TrafficController::init_for_test(policy_config(false, PolicyKind::Spam));
        controller
            .admin_reconfigure(TrafficControlReconfigParams {
                error_threshold: None,
                spam_threshold: None,
                dry_run: None,
                connection_blocklist_ttl_sec: Some(0),
                proxy_blocklist_ttl_sec: Some(0),
            })
            .expect("a zero blocklist TTL is valid");

        let log = capture_plain(|| controller.tally(breach(PolicyKind::Spam)));
        assert_eq!(log.block_lines(), Vec::<String>::new());
        assert!(controller.check(&Some(CLIENT), &None));
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

    fn allowlist_mode() -> TrafficController {
        TrafficController::init_for_test(PolicyConfig {
            allow_list: Some(vec![CLIENT.to_string()]),
            dry_run: false,
            ..Default::default()
        })
    }

    #[test]
    fn allowlist_mode_reports_no_blocklist_ttls() {
        let state = allowlist_mode().get_current_state();
        assert_eq!(state.connection_blocklist_ttl_sec, None);
        assert_eq!(state.proxy_blocklist_ttl_sec, None);
    }

    #[test]
    fn the_admin_api_rejects_a_blocklist_ttl_in_allowlist_mode() {
        let controller = allowlist_mode();
        for (connection_blocklist_ttl_sec, proxy_blocklist_ttl_sec) in
            [(Some(60), None), (None, Some(60))]
        {
            let result = controller.admin_reconfigure(TrafficControlReconfigParams {
                error_threshold: None,
                spam_threshold: None,
                dry_run: Some(true),
                connection_blocklist_ttl_sec,
                proxy_blocklist_ttl_sec,
            });
            assert!(matches!(result, Err(IotaError::InvalidAdminRequest(_))));
        }
        // A rejected request applies nothing, not even its valid settings.
        assert_eq!(controller.get_current_state().dry_run, Some(false));
    }

    #[test]
    fn the_admin_api_turns_dry_run_on_in_allowlist_mode() {
        let controller = allowlist_mode();
        let stranger = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 5));
        assert!(!controller.check(&Some(stranger), &None));

        controller
            .admin_reconfigure(TrafficControlReconfigParams {
                error_threshold: None,
                spam_threshold: None,
                dry_run: Some(true),
                connection_blocklist_ttl_sec: None,
                proxy_blocklist_ttl_sec: None,
            })
            .expect("allowlist mode has a dry-run flag");
        assert!(controller.check(&Some(stranger), &None));
    }
}
