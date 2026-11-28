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
mod simulation_gas;

use std::{
    fmt::Debug,
    net::{IpAddr, Ipv6Addr, SocketAddr},
    str,
    sync::Arc,
    time::Instant,
};

use iota_types::{
    error::IotaError,
    traffic_control::{
        AccessRuleList, AccessRules, BlockReason, BlockedClient, BlockedClients, ClientGasBalance,
        ClientIdSource, ClientUsage, DEFAULT_DENY_WARNING, DefaultAction,
        FORWARDED_CLIENT_IP_HEADER, PolicyConfig, PolicyType, TrafficControlReconfigParams,
        TrafficControlStatus,
    },
};
use tracing::{error, warn};

use self::{
    block_log::client_name,
    client_access::{ClientAccessManager, client_prefix, ipv6_prefix_len_of, parse_access_rule},
    metrics::TrafficControllerMetrics,
    simulation_gas::SimulationGasLimiter,
};
use crate::policies::{MAX_CLIENT_THRESHOLD, TrafficControlPolicy, TrafficTally};
pub use crate::simulation_gas::{SimulationGasRefusal, SimulationGasReservation};

/// The spam and error policies.
struct Policies {
    spam: TrafficControlPolicy,
    error: TrafficControlPolicy,
}

/// What [`TrafficController::check`] decided for a request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClientAccess {
    /// An allow rule matches the client: serve the request, and do not tally
    /// it, so that no policy counts or blocks the client.
    Allowed,
    /// Serve the request, and tally it.
    Limited,
    /// Refuse the request without handling it, and do not tally it.
    Refused,
}

impl ClientAccess {
    /// Whether to handle the request. It is false only for
    /// [`Self::Refused`]: the caller then sends the refusal and does not
    /// tally the request.
    pub fn is_served(self) -> bool {
        self != Self::Refused
    }
}

#[derive(Clone)]
pub struct TrafficController {
    client_access: Arc<ClientAccessManager>,
    policies: Arc<Policies>,
    simulation_gas: Arc<SimulationGasLimiter>,
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
    /// Must be called from within a tokio runtime, since it spawns the task
    /// that drops expired blocks.
    ///
    /// # Panics
    ///
    /// If an access rule is neither an IP address nor a CIDR network, if a
    /// blocklist TTL exceeds the maximum, if `ipv6-prefix-len` is out of
    /// range, if a `freq-threshold` policy has a `burst-secs` of 0, or if a
    /// value of `simulation-gas-limit` is 0.
    pub fn init(policy_config: PolicyConfig, metrics: Arc<TrafficControllerMetrics>) -> Self {
        let client_access = Arc::new(ClientAccessManager::new(&policy_config, metrics.clone()));
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
        let policies = Arc::new(policies);
        let simulation_gas = Arc::new(SimulationGasLimiter::new(
            policy_config.simulation_gas_limit.as_ref(),
            metrics.clone(),
        ));
        Self {
            client_access,
            policies,
            simulation_gas,
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

    /// The settings in force. A threshold is `None` for a policy that does
    /// not rate limit.
    pub fn get_current_state(&self) -> TrafficControlReconfigParams {
        let (connection_blocklist_ttl_sec, proxy_blocklist_ttl_sec) =
            self.client_access.blocklist_ttls_sec();
        let simulation_gas = self.simulation_gas.settings();
        TrafficControlReconfigParams {
            error_threshold: self.policies.error.client_threshold(),
            spam_threshold: self.policies.spam.client_threshold(),
            dry_run: Some(self.client_access.dry_run()),
            connection_blocklist_ttl_sec: Some(connection_blocklist_ttl_sec),
            proxy_blocklist_ttl_sec: Some(proxy_blocklist_ttl_sec),
            simulation_gas_per_sec: simulation_gas.as_ref().map(|limit| limit.gas_per_sec),
            simulation_burst_gas: simulation_gas.map(|limit| limit.burst_gas),
            default_action: Some(self.client_access.access_rules().default),
        }
    }

    /// The settings in force, with the number of rules and of blocked
    /// clients.
    pub fn status(&self) -> TrafficControlStatus {
        let (connection_blocklist_ttl_sec, proxy_blocklist_ttl_sec) =
            self.client_access.blocklist_ttls_sec();
        let rules = self.client_access.access_rules();
        TrafficControlStatus {
            error_threshold: self.policies.error.client_threshold(),
            spam_threshold: self.policies.spam.client_threshold(),
            dry_run: self.client_access.dry_run(),
            connection_blocklist_ttl_sec,
            proxy_blocklist_ttl_sec,
            default_action: rules.default,
            allow_rules: rules.allow.len(),
            deny_rules: rules.deny.len(),
            blocked_clients: self.client_access.blocked_len(),
            simulation_gas_limit: self.simulation_gas.settings(),
        }
    }

    /// The settings in force in the shape of `policy-config`: the config the
    /// node started with, with every change the admin API made since.
    pub fn runtime_policy_config(&self) -> PolicyConfig {
        let mut config = PolicyConfig::clone(&self.policy_config);
        let (connection_blocklist_ttl_sec, proxy_blocklist_ttl_sec) =
            self.client_access.blocklist_ttls_sec();
        config.dry_run = self.client_access.dry_run();
        config.connection_blocklist_ttl_sec = connection_blocklist_ttl_sec;
        config.proxy_blocklist_ttl_sec = proxy_blocklist_ttl_sec;
        for (policy_type, policy) in [
            (&mut config.spam_policy_type, &self.policies.spam),
            (&mut config.error_policy_type, &self.policies.error),
        ] {
            match (policy_type, policy.client_threshold()) {
                (PolicyType::FreqThreshold(freq), Some(threshold)) => {
                    freq.client_threshold = threshold
                }
                (PolicyType::TestNConnIP(count), Some(threshold)) => *count = threshold,
                _ => {}
            }
        }
        config.access_rules = self.client_access.access_rules();
        config.simulation_gas_limit = self.simulation_gas.settings();
        config
    }

    /// The client of a request that `client` sent with `headers`, and whether
    /// to serve the request, with dry run considered. A forwarder can name a
    /// user in one [`FORWARDED_CLIENT_IP_HEADER`] header. A deny rule or a
    /// block of the forwarder then refuses the request, unless an allow rule
    /// serves the forwarder. Otherwise the user is checked as [`Self::check`]
    /// checks a client, and is the client returned. Any other request is `client`'s
    /// own, checked as [`Self::check`] does. Tally the request to the client
    /// this returns.
    pub fn resolve_forwarded_client_and_check(
        &self,
        client: Option<IpAddr>,
        headers: &http::HeaderMap,
    ) -> (Option<IpAddr>, ClientAccess) {
        let forwarded: Vec<&[u8]> = headers
            .get_all(FORWARDED_CLIENT_IP_HEADER)
            .iter()
            .map(http::HeaderValue::as_bytes)
            .collect();
        self.client_access
            .resolve_forwarded_client_and_check(client, &forwarded)
    }

    /// The access rules in force, each in its canonical form.
    pub fn access_rules(&self) -> AccessRules {
        self.client_access.access_rules()
    }

    /// The clients the node refuses until their blocks expire, at most `limit`
    /// of them.
    pub fn blocklist(&self, limit: usize) -> BlockedClients {
        self.client_access.blocklist(limit)
    }

    /// Refuses the requests whose client is `client` for `ttl_sec` seconds,
    /// with the reason `manual`, and returns its block in force. The client of
    /// a request is the address that connects to this node, the one that
    /// `client-id-source` names, or a user that a trusted forwarder names.
    /// Like the access rules, it does not refuse requests that reach this node
    /// through a fullnode. `client` is an IP address, or an IPv6 prefix as the
    /// blocklist names it; an IPv6 address is blocked with its whole
    /// `ipv6-prefix-len` prefix. Logs the block line outside dry run. A block in force that lasts longer keeps its
    /// expiration and its reason. The block lasts until it expires or the node
    /// restarts. Under dry run the node still serves the client.
    ///
    /// # Errors
    ///
    /// `InvalidAdminRequest` when `client` is neither an IP address nor an
    /// IPv6 prefix as the blocklist names it, when `ttl_sec` is 0 or above the
    /// longest blocklist TTL, or when an allow rule serves `client`.
    pub fn block_client_manually(
        &self,
        client: &str,
        ttl_sec: u64,
    ) -> Result<BlockedClient, IotaError> {
        let client = self.parse_client(client)?;
        self.client_access
            .block_manually(client, ttl_sec)
            .map_err(IotaError::InvalidAdminRequest)
    }

    /// Whether dry run is on, in which case the node serves every client.
    pub fn dry_run(&self) -> bool {
        self.client_access.dry_run()
    }

    /// Drops the blocks of `client`, as a direct client and as one that
    /// reached this node through a fullnode, and returns whether it had one in
    /// force. `client` is an IP address, or an IPv6 prefix as the blocklist
    /// names it. Also drops what the spam and error policies counted for
    /// the client, and its simulation gas balance, also when it had no block,
    /// so that its next request does not block it again at once. Logs the
    /// unblock line for a block whose block line was logged.
    ///
    /// # Errors
    ///
    /// `InvalidAdminRequest` when `client` is neither an IP address nor an
    /// IPv6 prefix as the blocklist names it.
    pub fn unblock_client(&self, client: &str) -> Result<bool, IotaError> {
        let client = self.parse_client(client)?;
        self.policies.spam.forget(client);
        self.policies.error.forget(client);
        self.simulation_gas.forget(Some(client));
        Ok(self.client_access.unblock(client))
    }

    /// The `top` clients with the lowest simulation gas balances, lowest
    /// first, and up to `top` blocked clients. What the spam and error policies
    /// counted is not listed.
    pub fn client_usage(&self, top: usize) -> ClientUsage {
        let (balances, omitted_gas_balances) =
            self.simulation_gas.lowest_balances(top, Instant::now());
        let ipv6_prefix_len = self.policy_config.ipv6_prefix_len;
        ClientUsage {
            gas_balances: balances
                .into_iter()
                .map(|(client, gas)| ClientGasBalance {
                    client: client.map(|client| client_name(client, ipv6_prefix_len)),
                    gas,
                })
                .collect(),
            omitted_gas_balances,
            blocked: self.client_access.blocklist(top),
        }
    }

    /// The blocklist key of `client`: an IP address, or an IPv6 prefix with
    /// its length as the blocklist names it, such as `2001:db8:1:2::/64`.
    fn parse_client(&self, client: &str) -> Result<IpAddr, IotaError> {
        let ipv6_prefix_len = self.policy_config.ipv6_prefix_len;
        let invalid = || {
            IotaError::InvalidAdminRequest(format!(
                "{client:?} is not an IP address, nor an IPv6 prefix as the blocklist names it"
            ))
        };
        let address = match client.split_once('/') {
            None => client.parse::<IpAddr>().map_err(|_| invalid())?,
            Some((address, prefix_len)) => {
                let address = address.parse::<Ipv6Addr>().map_err(|_| invalid())?;
                let prefix_len = prefix_len.parse::<u8>().map_err(|_| invalid())?;
                if prefix_len != ipv6_prefix_len_of(address, ipv6_prefix_len) {
                    return Err(invalid());
                }
                IpAddr::V6(address)
            }
        };
        Ok(client_prefix(address, ipv6_prefix_len))
    }

    /// Sets the computation limit of one transaction,
    /// `max_gas_computation_bucket` of the protocol in force, in gas units. A
    /// simulation reserves this much of its client's gas balance, or
    /// `burst-gas` if that is lower, and is charged at most this much. Until
    /// it is set, a simulation reserves the whole `burst-gas`.
    pub fn set_max_simulation_computation(&self, gas: u64) {
        self.simulation_gas.set_max_computation(gas);
    }

    /// Reserves gas for one simulation of `client`, which [`Self::check`]
    /// decided as `access`. Returns `None` when there is nothing to reserve:
    /// simulations have no gas limit, or an allow rule serves the client.
    /// In dry run a client over the limit is served, and counted.
    ///
    /// # Errors
    ///
    /// [`SimulationGasRefusal`] while the client's balance is below the
    /// reservation: the computation limit of one transaction, or `burst-gas`
    /// if that is lower.
    pub fn reserve_simulation_gas(
        &self,
        client: Option<IpAddr>,
        access: ClientAccess,
    ) -> Result<Option<SimulationGasReservation>, SimulationGasRefusal> {
        if access == ClientAccess::Allowed {
            return Ok(None);
        }
        let client = client.map(|client| client_prefix(client, self.policy_config.ipv6_prefix_len));
        let Some(reserved) =
            self.simulation_gas
                .reserve(client, self.client_access.dry_run(), Instant::now())?
        else {
            return Ok(None);
        };
        Ok(Some(SimulationGasReservation::new(
            self.simulation_gas.clone(),
            client,
            reserved,
        )))
    }

    /// Adds an access rule, an IP address or a CIDR network, to `list`, and
    /// returns whether the list did not hold it already. The next check
    /// applies it. The rule lasts until the node restarts.
    ///
    /// # Errors
    ///
    /// [`IotaError::InvalidAdminRequest`] if `rule` is neither an IP address
    /// nor a CIDR network.
    pub fn add_access_rule(&self, list: AccessRuleList, rule: &str) -> Result<bool, IotaError> {
        let rule = parse_access_rule(rule)
            .map_err(|err| IotaError::InvalidAdminRequest(format!("{rule:?}: {err}")))?;
        Ok(self.client_access.add_rule(list, rule))
    }

    /// Removes an access rule from `list`, and returns whether the list held
    /// it. The rule is compared in its canonical form, so `198.51.100.7/24`
    /// removes `198.51.100.0/24`.
    ///
    /// # Errors
    ///
    /// [`IotaError::InvalidAdminRequest`] if `rule` is neither an IP address
    /// nor a CIDR network.
    pub fn remove_access_rule(&self, list: AccessRuleList, rule: &str) -> Result<bool, IotaError> {
        let rule = parse_access_rule(rule)
            .map_err(|err| IotaError::InvalidAdminRequest(format!("{rule:?}: {err}")))?;
        Ok(self.client_access.remove_rule(list, rule))
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
            simulation_gas_per_sec,
            simulation_burst_gas,
            default_action,
        } = params;
        let updates = [
            (
                error_threshold,
                &self.policies.error,
                &self.metrics.error_client_threshold,
            ),
            (
                spam_threshold,
                &self.policies.spam,
                &self.metrics.spam_client_threshold,
            ),
        ];
        // Validate the whole request first, so a rejected one applies nothing.
        for (threshold, policy, _) in updates {
            if let Some(threshold) = threshold {
                validate_threshold(policy, threshold)?;
            }
        }
        self.client_access
            .validate_reconfigure(connection_blocklist_ttl_sec, proxy_blocklist_ttl_sec)
            .map_err(IotaError::InvalidAdminRequest)?;
        self.simulation_gas
            .validate_reconfigure(simulation_gas_per_sec, simulation_burst_gas)
            .map_err(IotaError::InvalidAdminRequest)?;
        // Dry run goes on before the other changes and off after them, so that a
        // request checked meanwhile never meets a mix of the old and the new
        // settings that refuses more than both.
        let turns_dry_run_on = dry_run.filter(|dry_run| *dry_run);
        let turns_dry_run_off = dry_run.filter(|dry_run| !*dry_run);
        self.client_access.reconfigure(turns_dry_run_on, None, None);
        for (threshold, policy, gauge) in updates {
            if let Some(threshold) = threshold {
                policy.set_client_threshold(threshold);
                gauge.set(threshold as i64);
            }
        }
        self.simulation_gas
            .reconfigure(simulation_gas_per_sec, simulation_burst_gas);
        if let Some(default_action) = default_action {
            let changed = self.client_access.set_default_action(default_action);
            if changed && default_action == DefaultAction::Deny {
                warn!("The admin API set the default action deny. {DEFAULT_DENY_WARNING}");
            }
        }
        self.client_access.reconfigure(
            turns_dry_run_off,
            connection_blocklist_ttl_sec,
            proxy_blocklist_ttl_sec,
        );

        Ok(self.get_current_state())
    }

    /// Charges the tally against the spam and error policies, and has the
    /// client access manager refuse each client that breaches one, before
    /// returning. An IPv6 client is charged for the first `ipv6-prefix-len`
    /// bits of its address. Do not tally a request that [`Self::check`]
    /// decided as [`ClientAccess::Allowed`].
    pub fn tally(&self, mut tally: TrafficTally) {
        let policies = &self.policies;
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

    /// Whether to serve a request from `client`, and whether to tally it, with
    /// dry run considered. The access rules match the full address of
    /// `client`. A request whose client IP the node could not resolve matches
    /// no rule, so the default action decides it.
    pub fn check(&self, client: &Option<IpAddr>, proxied_client: &Option<IpAddr>) -> ClientAccess {
        self.client_access.check(client, proxied_client)
    }
}

fn validate_threshold(policy: &TrafficControlPolicy, threshold: u64) -> Result<(), IotaError> {
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

/// `rules` with each rule in the form that the admin API reports it in, such
/// as `203.0.113.7/32` for `203.0.113.7`.
///
/// # Errors
///
/// For a rule that is neither an IP address nor a CIDR network, the rule and
/// why it does not parse.
pub fn canonical_access_rules(rules: &AccessRules) -> Result<AccessRules, String> {
    let canonical = |rules: &[String]| {
        rules
            .iter()
            .map(|rule| {
                parse_access_rule(rule)
                    .map(|network| network.to_string())
                    .map_err(|err| format!("access rule {rule:?}: {err}"))
            })
            .collect::<Result<Vec<_>, _>>()
    };
    Ok(AccessRules {
        allow: canonical(&rules.allow)?,
        deny: canonical(&rules.deny)?,
        default: rules.default,
        forwarders: canonical(&rules.forwarders)?,
    })
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

impl ClientIpStatus {
    /// Whether the server refuses a request with this outcome.
    pub fn should_refuse_request(&self) -> bool {
        matches!(
            self,
            Self::SocketAddrMissing | Self::XForwardedForInvalidUtf8
        )
    }
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
    use iota_types::traffic_control::{
        AccessRules, DefaultAction, FreqThresholdConfig, SimulationGasLimitConfig, Weight,
        default_blocklist_ttl_sec,
    };
    use tracing_subscriber::layer::SubscriberExt;

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
        assert!(controller.check(&Some(CLIENT), &None).is_served());
        assert_eq!(controller.metrics.num_dry_run_blocked_requests.get(), 1);
    }

    #[tokio::test]
    async fn test_the_admin_api_turns_dry_run_off_at_once() {
        let controller = TrafficController::init_for_test(policy_config(true, PolicyKind::Spam));
        controller.tally(breach(PolicyKind::Spam));
        assert!(controller.check(&Some(CLIENT), &None).is_served());

        controller
            .admin_reconfigure(TrafficControlReconfigParams {
                error_threshold: None,
                spam_threshold: None,
                dry_run: Some(false),
                connection_blocklist_ttl_sec: None,
                proxy_blocklist_ttl_sec: None,
                ..Default::default()
            })
            .expect("the request changes only the dry-run flag");

        // The next check must read the new value, not the value at startup.
        assert!(!controller.check(&Some(CLIENT), &None).is_served());
    }

    #[tokio::test]
    async fn test_dry_run_reports_an_error_block_it_does_not_apply() {
        let controller = TrafficController::init_for_test(policy_config(true, PolicyKind::Error));
        controller.tally(breach(PolicyKind::Error));
        assert_eq!(controller.metrics.connection_ip_blocklist_len.get(), 1);
        assert!(controller.check(&Some(CLIENT), &None).is_served());
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
                    ..Default::default()
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
        assert!(!controller.check(&Some(address(3)), &None).is_served());
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
        assert!(!controller.check(&None, &Some(CLIENT)).is_served());
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
                ..Default::default()
            })
            .expect("the request changes only the connection blocklist TTL");

        // The next block must take the new TTL, not the TTL at startup.
        let log = capture_plain(|| controller.tally(breach(PolicyKind::Spam)));
        let [line] = log.block_lines().try_into().unwrap();
        assert!(line.ends_with(" ttl_secs=120"), "{line}");
        assert!(!controller.check(&Some(CLIENT), &None).is_served());
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
                ..Default::default()
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
                ..Default::default()
            })
            .expect("a zero blocklist TTL is valid");

        let log = capture_plain(|| controller.tally(breach(PolicyKind::Spam)));
        assert_eq!(log.block_lines(), Vec::<String>::new());
        assert!(controller.check(&Some(CLIENT), &None).is_served());
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
        assert!(controller.check(&Some(CLIENT), &None).is_served());

        // Over the budget the shipped blocklist TTL keeps the block in place,
        // thus a later check rejects the client. The bound leaves room for the
        // cells the limiter replenishes while the test runs.
        for _ in 0..budget {
            controller.tally(breach(PolicyKind::Error));
            if !controller.check(&Some(CLIENT), &None).is_served() {
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
            ..Default::default()
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
        let status = get_client_ip(&headers, None, &one_hop());
        assert!(matches!(status, ClientIpStatus::XForwardedForInvalidUtf8));
        assert!(status.should_refuse_request());
    }

    #[test]
    fn a_selected_entry_that_is_not_an_address_resolves_no_client() {
        let headers = forwarded(b"10.0.0.1, not-an-address");
        let status = get_client_ip(&headers, None, &one_hop());
        assert!(matches!(status, ClientIpStatus::XForwardedForUnparsable));
        assert!(!status.should_refuse_request());
    }

    #[test]
    fn a_request_without_a_peer_address_is_refused() {
        let status = get_client_ip(&http::HeaderMap::new(), None, &ClientIdSource::SocketAddr);
        assert!(matches!(status, ClientIpStatus::SocketAddrMissing));
        assert!(status.should_refuse_request());
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

    #[tokio::test]
    async fn rate_limiting_admits_a_request_with_an_unresolved_client_ip() {
        let controller = TrafficController::init_for_test(policy_config(false, PolicyKind::Spam));
        // The policy blocks every client it is charged, and a request with no
        // resolved client IP is charged to none of them.
        controller.tally(breach(PolicyKind::Spam));
        assert!(!controller.check(&Some(CLIENT), &None).is_served());
        assert!(controller.check(&None, &None).is_served());
        // Only the request with no resolved client IP is counted.
        assert_eq!(controller.metrics.unresolved_client_requests.get(), 1);
    }

    fn gas_limited(dry_run: bool, allow: Vec<String>) -> TrafficController {
        let controller = TrafficController::init_for_test(PolicyConfig {
            access_rules: AccessRules {
                allow,
                ..Default::default()
            },
            simulation_gas_limit: Some(SimulationGasLimitConfig {
                gas_per_sec: 1,
                burst_gas: 100,
            }),
            dry_run,
            ..Default::default()
        });
        controller.set_max_simulation_computation(100);
        controller
    }

    #[tokio::test]
    async fn a_simulation_is_refused_once_the_balance_is_used() {
        let controller = gas_limited(false, vec![]);
        let reservation = controller
            .reserve_simulation_gas(Some(CLIENT), ClientAccess::Limited)
            .unwrap()
            .expect("the limit is on");
        reservation.settle(100_000, 1000);
        let refusal = controller
            .reserve_simulation_gas(Some(CLIENT), ClientAccess::Limited)
            .expect_err("the balance is used");
        // 100 gas is missing. The balance refills by 1 gas per second, so a
        // stall between the two reservations shortens the wait by as many
        // seconds.
        let retry_after_secs = refusal.retry_after().as_secs();
        assert!((95..=100).contains(&retry_after_secs), "{retry_after_secs}");
        assert_eq!(
            refusal.to_string(),
            format!("simulation gas limit reached, retry in {retry_after_secs}s")
        );
    }

    #[tokio::test]
    async fn a_dropped_reservation_gives_all_of_it_back() {
        let controller = gas_limited(false, vec![]);
        drop(controller.reserve_simulation_gas(Some(CLIENT), ClientAccess::Limited));
        assert!(
            controller
                .reserve_simulation_gas(Some(CLIENT), ClientAccess::Limited)
                .is_ok()
        );
    }

    #[tokio::test]
    async fn an_allowed_client_has_no_gas_limit() {
        let controller = gas_limited(false, vec![CLIENT.to_string()]);
        let access = controller.check(&Some(CLIENT), &None);
        assert_eq!(access, ClientAccess::Allowed);
        for _ in 0..3 {
            assert!(matches!(
                controller.reserve_simulation_gas(Some(CLIENT), access),
                Ok(None)
            ));
        }
    }

    /// The addresses of one IPv6 prefix of `ipv6-prefix-len` bits share one
    /// balance.
    #[tokio::test]
    async fn the_addresses_of_one_ipv6_prefix_share_a_balance() {
        let controller = gas_limited(false, vec![]);
        let address =
            |last: u16| IpAddr::V6(std::net::Ipv6Addr::new(0x2001, 0xdb8, 1, 2, 0, 0, 0, last));
        let _held = controller
            .reserve_simulation_gas(Some(address(1)), ClientAccess::Limited)
            .unwrap();
        assert!(
            controller
                .reserve_simulation_gas(Some(address(2)), ClientAccess::Limited)
                .is_err()
        );
        let other_prefix = IpAddr::V6(std::net::Ipv6Addr::new(0x2001, 0xdb8, 1, 3, 0, 0, 0, 1));
        assert!(
            controller
                .reserve_simulation_gas(Some(other_prefix), ClientAccess::Limited)
                .is_ok()
        );
    }

    #[tokio::test]
    async fn dry_run_serves_a_simulation_over_the_gas_limit() {
        let controller = gas_limited(true, vec![]);
        let _held = controller.reserve_simulation_gas(Some(CLIENT), ClientAccess::Limited);
        assert!(
            controller
                .reserve_simulation_gas(Some(CLIENT), ClientAccess::Limited)
                .is_ok()
        );
        assert_eq!(
            controller.metrics.num_dry_run_simulation_gas_refusals.get(),
            1
        );
    }

    #[tokio::test]
    async fn a_request_with_a_rejected_simulation_gas_value_applies_nothing() {
        let controller = TrafficController::init_for_test(PolicyConfig {
            spam_policy_type: freq_threshold(100),
            simulation_gas_limit: Some(SimulationGasLimitConfig {
                gas_per_sec: 1,
                burst_gas: 100,
            }),
            dry_run: false,
            ..Default::default()
        });
        // `gas-per-sec` of 0 is rejected. The other values are valid, but the
        // controller must apply none of them.
        let result = controller.admin_reconfigure(TrafficControlReconfigParams {
            spam_threshold: Some(10),
            dry_run: Some(true),
            connection_blocklist_ttl_sec: Some(5),
            simulation_gas_per_sec: Some(0),
            simulation_burst_gas: Some(50),
            ..Default::default()
        });

        assert!(matches!(result, Err(IotaError::InvalidAdminRequest(_))));
        let state = controller.get_current_state();
        assert_eq!(state.spam_threshold, Some(100));
        assert_eq!(state.dry_run, Some(false));
        assert_eq!(
            state.connection_blocklist_ttl_sec,
            Some(default_blocklist_ttl_sec())
        );
        assert_eq!(state.simulation_gas_per_sec, Some(1));
        assert_eq!(state.simulation_burst_gas, Some(100));
    }

    #[tokio::test]
    async fn the_admin_api_changes_the_simulation_gas_limit() {
        let controller = TrafficController::init_for_test(PolicyConfig::default());
        assert!(matches!(
            controller.admin_reconfigure(TrafficControlReconfigParams {
                simulation_gas_per_sec: Some(10),
                ..Default::default()
            }),
            Err(IotaError::InvalidAdminRequest(_))
        ));
        let state = controller
            .admin_reconfigure(TrafficControlReconfigParams {
                simulation_gas_per_sec: Some(10),
                simulation_burst_gas: Some(20),
                ..Default::default()
            })
            .expect("both values turn the limit on");
        assert_eq!(state.simulation_gas_per_sec, Some(10));
        assert_eq!(state.simulation_burst_gas, Some(20));
    }

    #[tokio::test]
    async fn a_manual_block_refuses_the_client_and_logs_it() {
        let controller = TrafficController::init_for_test(policy_config(false, PolicyKind::Spam));
        let mut block = None;
        let log = capture_plain(|| {
            block = Some(controller.block_client_manually("10.0.0.1", 300).unwrap())
        });
        let block = block.unwrap();
        let [line] = log.block_lines().try_into().unwrap();
        assert!(
            line.ends_with("client_blocked client=10.0.0.1 reason=manual ttl_secs=300"),
            "{line}"
        );
        assert_eq!(
            controller.check(&Some(CLIENT), &None),
            ClientAccess::Refused
        );
        assert_eq!(
            (block.client.as_str(), block.proxied, block.reason),
            ("10.0.0.1", false, BlockReason::Manual)
        );
        assert!((299..=300).contains(&block.seconds_left), "{block:?}");
        let [listed] = controller.blocklist(10).clients.try_into().unwrap();
        assert_eq!(
            (listed.client.as_str(), listed.proxied, listed.reason),
            ("10.0.0.1", false, BlockReason::Manual)
        );
        assert!((299..=300).contains(&listed.seconds_left), "{listed:?}");
    }

    #[tokio::test]
    async fn the_admin_api_rejects_an_invalid_manual_block() {
        let controller = TrafficController::init_for_test(policy_config(false, PolicyKind::Spam));
        for (client, ttl_sec) in [
            ("10.0.0.1", 0),
            ("10.0.0.1", MAX_BLOCKLIST_TTL_SEC + 1),
            ("not-an-address", 60),
            ("10.0.0.0/24", 60),
            // The blocklist names this client 2001:db8:1:2::/64.
            ("2001:db8:1:2::/48", 60),
        ] {
            assert!(matches!(
                controller.block_client_manually(client, ttl_sec),
                Err(IotaError::InvalidAdminRequest(_))
            ));
        }
        assert_eq!(controller.blocklist(10).clients, vec![]);
    }

    #[tokio::test]
    async fn a_manual_block_of_a_client_an_allow_rule_serves_is_rejected() {
        let controller = TrafficController::init_for_test(PolicyConfig {
            access_rules: AccessRules {
                allow: vec!["10.0.0.0/24".to_string(), "2001:db8:1:2::7".to_string()],
                ..Default::default()
            },
            ..policy_config(false, PolicyKind::Spam)
        });
        let log = capture_plain(|| {
            for client in ["10.0.0.1", "2001:db8:1:2::5"] {
                let Err(IotaError::InvalidAdminRequest(message)) =
                    controller.block_client_manually(client, 60)
                else {
                    panic!("the block of {client} was not rejected");
                };
                assert!(message.contains("remove the rule first"), "{message}");
            }
        });
        assert_eq!(log.block_lines(), Vec::<String>::new());
        assert_eq!(controller.blocklist(10).clients, vec![]);
        controller.block_client_manually("10.0.1.1", 60).unwrap();
    }

    #[tokio::test]
    async fn a_manual_block_that_outlasts_a_logged_block_is_logged_again() {
        let controller = TrafficController::init_for_test(policy_config(false, PolicyKind::Spam));
        let log = capture_plain(|| {
            controller.tally(breach(PolicyKind::Spam));
            let shorter = controller.block_client_manually("10.0.0.1", 60).unwrap();
            assert_eq!(shorter.reason, BlockReason::Spam);
            assert!((119..=120).contains(&shorter.seconds_left), "{shorter:?}");
            let longer = controller.block_client_manually("10.0.0.1", 3600).unwrap();
            assert_eq!(longer.reason, BlockReason::Manual);
            assert!((3599..=3600).contains(&longer.seconds_left), "{longer:?}");
        });
        let lines = log.block_lines();
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert!(lines[0].ends_with("reason=spam ttl_secs=120"), "{lines:?}");
        assert!(
            lines[1].ends_with("reason=manual ttl_secs=3600"),
            "{lines:?}"
        );
    }

    /// Sets dry run through the admin API.
    fn set_dry_run(controller: &TrafficController, dry_run: bool) {
        controller
            .admin_reconfigure(TrafficControlReconfigParams {
                dry_run: Some(dry_run),
                ..Default::default()
            })
            .unwrap();
    }

    #[tokio::test]
    async fn a_longer_manual_block_under_dry_run_is_logged_when_dry_run_goes_off() {
        let controller = TrafficController::init_for_test(policy_config(false, PolicyKind::Spam));
        let log = capture_plain(|| {
            controller.tally(breach(PolicyKind::Spam));
            set_dry_run(&controller, true);
            controller.block_client_manually("10.0.0.1", 3600).unwrap();
            set_dry_run(&controller, false);
        });
        let lines = log.block_lines();
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert!(lines[0].ends_with("reason=spam ttl_secs=120"), "{lines:?}");
        let (_, ttl_secs) = lines[1]
            .split_once("client_blocked client=10.0.0.1 reason=manual ttl_secs=")
            .expect("the second line logs the manual block");
        assert!(["3599", "3600"].contains(&ttl_secs), "{lines:?}");
    }

    #[tokio::test]
    async fn an_unblock_under_dry_run_lifts_the_logged_ban_that_a_longer_manual_block_outlasts() {
        let controller = TrafficController::init_for_test(policy_config(false, PolicyKind::Spam));
        let log = capture_plain(|| {
            controller.tally(breach(PolicyKind::Spam));
            set_dry_run(&controller, true);
            controller.block_client_manually("10.0.0.1", 3600).unwrap();
            assert!(controller.unblock_client("10.0.0.1").unwrap());
        });
        assert!(
            log.lines()
                .iter()
                .any(|line| line.ends_with("client_unblocked client=10.0.0.1")),
            "{:?}",
            log.lines()
        );
    }

    #[tokio::test]
    async fn an_ipv6_client_is_blocked_and_unblocked_by_the_name_the_blocklist_gives() {
        let controller = TrafficController::init_for_test(policy_config(false, PolicyKind::Spam));
        let block = controller
            .block_client_manually("2001:db8:1:2::5", 60)
            .unwrap();
        assert_eq!(block.client, "2001:db8:1:2::/64");
        let other = IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 1, 2, 0, 0, 0, 9));
        assert_eq!(controller.check(&Some(other), &None), ClientAccess::Refused);

        assert!(controller.unblock_client(&block.client).unwrap());
        assert!(controller.check(&Some(other), &None).is_served());
    }

    #[tokio::test]
    async fn an_unblocked_client_starts_again_with_its_whole_burst() {
        let controller = TrafficController::init_for_test(PolicyConfig {
            spam_policy_type: PolicyType::TestNConnIP(3),
            ..policy_config(false, PolicyKind::Spam)
        });
        let tally = || TrafficTally::new(Some(CLIENT), None, None, Weight::one());
        for _ in 0..3 {
            controller.tally(tally());
        }
        assert!(!controller.check(&Some(CLIENT), &None).is_served());

        let log = capture_plain(|| assert!(controller.unblock_client("10.0.0.1").unwrap()));
        assert!(
            log.lines()
                .iter()
                .any(|line| line.ends_with("client_unblocked client=10.0.0.1")),
            "{:?}",
            log.lines()
        );
        assert!(controller.check(&Some(CLIENT), &None).is_served());
        // The policy forgot the tallies before the unblock.
        controller.tally(tally());
        controller.tally(tally());
        assert!(controller.check(&Some(CLIENT), &None).is_served());
        assert!(!controller.unblock_client("10.0.0.1").unwrap());
    }

    #[tokio::test]
    async fn an_unblock_forgets_the_errors_of_the_client() {
        let controller = TrafficController::init_for_test(PolicyConfig {
            error_policy_type: PolicyType::TestNConnIP(2),
            ..policy_config(false, PolicyKind::Error)
        });
        for _ in 0..2 {
            controller.tally(breach(PolicyKind::Error));
        }
        assert!(!controller.check(&Some(CLIENT), &None).is_served());

        assert!(controller.unblock_client("10.0.0.1").unwrap());
        controller.tally(breach(PolicyKind::Error));
        assert!(controller.check(&Some(CLIENT), &None).is_served());
    }

    #[tokio::test]
    async fn an_unblock_gives_the_client_a_full_gas_balance() {
        let controller = gas_limited(false, vec![]);
        controller
            .reserve_simulation_gas(Some(CLIENT), ClientAccess::Limited)
            .unwrap()
            .unwrap()
            .settle(60, 1);
        assert_eq!(controller.client_usage(10).gas_balances.len(), 1);

        assert!(!controller.unblock_client("10.0.0.1").unwrap());
        assert_eq!(controller.client_usage(10).gas_balances, vec![]);
    }

    #[tokio::test]
    async fn an_unblock_drops_the_block_of_a_client_behind_a_fullnode() {
        let controller = TrafficController::init_for_test(PolicyConfig {
            spam_policy_type: freq_threshold(1),
            ..policy_config(false, PolicyKind::Spam)
        });
        let tally = || TrafficTally::new(None, Some(CLIENT), None, Weight::one());
        let mut burst = 0;
        while controller.check(&None, &Some(CLIENT)).is_served() {
            assert!(burst < 100, "the policy never blocked the client");
            controller.tally(tally());
            burst += 1;
        }
        assert_eq!(controller.status().blocked_clients, 1);

        let log = capture_plain(|| assert!(controller.unblock_client("10.0.0.1").unwrap()));
        assert!(controller.check(&None, &Some(CLIENT)).is_served());
        assert_eq!(controller.status().blocked_clients, 0);
        // The node never logged a block line for it.
        assert!(
            !log.lines()
                .iter()
                .any(|line| line.contains("client_unblocked")),
            "{:?}",
            log.lines()
        );
        // The policy forgot the tallies before the unblock.
        for _ in 1..burst {
            controller.tally(tally());
        }
        assert!(controller.check(&None, &Some(CLIENT)).is_served());
    }

    #[tokio::test]
    async fn a_client_blocked_directly_and_behind_a_fullnode_counts_as_one_blocked_client() {
        let controller = TrafficController::init_for_test(PolicyConfig {
            spam_policy_type: freq_threshold(0),
            ..policy_config(false, PolicyKind::Spam)
        });
        controller.tally(TrafficTally::new(None, Some(CLIENT), None, Weight::one()));
        controller.block_client_manually("10.0.0.1", 60).unwrap();
        assert_eq!(controller.blocklist(10).clients.len(), 2);
        assert_eq!(controller.status().blocked_clients, 1);
    }

    #[tokio::test]
    async fn a_manual_block_under_dry_run_is_kept_and_its_unblock_is_not_logged() {
        let controller = TrafficController::init_for_test(policy_config(true, PolicyKind::Spam));
        let log = capture_plain(|| {
            controller.block_client_manually("10.0.0.1", 300).unwrap();
            assert!(controller.dry_run());
            assert!(controller.check(&Some(CLIENT), &None).is_served());
            assert_eq!(controller.blocklist(10).clients.len(), 1);
            assert!(controller.unblock_client("10.0.0.1").unwrap());
        });
        assert_eq!(log.lines(), Vec::<String>::new());
    }

    #[tokio::test]
    async fn the_admin_api_warns_when_it_sets_the_default_deny() {
        let controller = TrafficController::init_for_test(policy_config(false, PolicyKind::Spam));
        let deny = || {
            controller
                .admin_reconfigure(TrafficControlReconfigParams {
                    default_action: Some(DefaultAction::Deny),
                    ..Default::default()
                })
                .unwrap()
        };
        let warned = |log: &block_log::tests::CapturedLog| {
            log.lines().iter().any(|line| {
                line.contains("WARN")
                    && line.contains("The admin API set the default action deny.")
                    && line.contains(DEFAULT_DENY_WARNING)
            })
        };
        assert!(warned(&capture_plain(|| {
            deny();
        })));
        assert!(!warned(&capture_plain(|| {
            deny();
        })));
    }

    #[tokio::test]
    async fn a_listing_says_how_many_entries_it_left_out() {
        let controller = TrafficController::init_for_test(policy_config(false, PolicyKind::Spam));
        for last in 1..=5 {
            controller
                .block_client_manually(&format!("10.0.0.{last}"), 60)
                .unwrap();
        }
        let blocklist = controller.blocklist(2);
        let clients = blocklist
            .clients
            .iter()
            .map(|block| block.client.as_str())
            .collect::<Vec<_>>();
        assert_eq!(clients, ["10.0.0.1", "10.0.0.2"]);
        assert_eq!(blocklist.omitted, 3);
        assert_eq!(controller.status().blocked_clients, 5);
    }

    #[tokio::test]
    async fn the_runtime_config_holds_the_changes_of_the_admin_api() {
        let controller = TrafficController::init_for_test(PolicyConfig {
            spam_policy_type: freq_threshold(100),
            dry_run: true,
            ..Default::default()
        });
        controller
            .admin_reconfigure(TrafficControlReconfigParams {
                spam_threshold: Some(200),
                dry_run: Some(false),
                connection_blocklist_ttl_sec: Some(90),
                default_action: Some(DefaultAction::Deny),
                simulation_gas_per_sec: Some(10),
                simulation_burst_gas: Some(20),
                ..Default::default()
            })
            .unwrap();
        controller
            .add_access_rule(AccessRuleList::Allow, "198.51.100.7/24")
            .unwrap();

        let config = controller.runtime_policy_config();
        let PolicyType::FreqThreshold(spam) = &config.spam_policy_type else {
            panic!("the spam policy rate limits");
        };
        assert_eq!(spam.client_threshold, 200);
        assert!(!config.dry_run);
        assert_eq!(config.connection_blocklist_ttl_sec, 90);
        assert_eq!(
            config.access_rules,
            AccessRules {
                allow: vec!["198.51.100.0/24".to_string()],
                deny: vec![],
                default: DefaultAction::Deny,
                forwarders: vec![],
            }
        );
        assert_eq!(
            config.simulation_gas_limit,
            Some(SimulationGasLimitConfig {
                gas_per_sec: 10,
                burst_gas: 20
            })
        );
        let status = controller.status();
        assert_eq!(status.default_action, DefaultAction::Deny);
        assert_eq!(status.allow_rules, 1);
    }

    #[tokio::test]
    async fn the_admin_api_sets_the_default_action() {
        let controller = TrafficController::init_for_test(policy_config(false, PolicyKind::Spam));
        assert!(controller.check(&Some(CLIENT), &None).is_served());
        controller
            .admin_reconfigure(TrafficControlReconfigParams {
                default_action: Some(DefaultAction::Deny),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(
            controller.check(&Some(CLIENT), &None),
            ClientAccess::Refused
        );
    }

    /// Asks `controller` about a client that no rule names at each event of
    /// `target` at `level`, and keeps what it decided.
    struct CheckOnEvent {
        controller: TrafficController,
        target: &'static str,
        level: tracing::Level,
        decisions: Arc<std::sync::Mutex<Vec<ClientAccess>>>,
    }

    impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for CheckOnEvent {
        fn on_event(
            &self,
            event: &tracing::Event<'_>,
            _: tracing_subscriber::layer::Context<'_, S>,
        ) {
            let metadata = event.metadata();
            if metadata.target() == self.target && *metadata.level() == self.level {
                let stranger = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 5));
                let decision = self.controller.check(&Some(stranger), &None);
                self.decisions.lock().unwrap().push(decision);
            }
        }
    }

    /// What `controller` decides for a client that no rule names at each event
    /// of `target` at `level` that `change` logs.
    fn collect_decisions_at_events(
        controller: &TrafficController,
        target: &'static str,
        level: tracing::Level,
        change: impl FnOnce(),
    ) -> Vec<ClientAccess> {
        let decisions = Arc::default();
        let subscriber = tracing_subscriber::registry().with(CheckOnEvent {
            controller: controller.clone(),
            target,
            level,
            decisions: Arc::clone(&decisions),
        });
        tracing::subscriber::with_default(subscriber, change);
        let mut decisions = decisions.lock().unwrap();
        std::mem::take(&mut *decisions)
    }

    /// Tallies a breach of `client` at the first block log line that the
    /// subscriber sees.
    struct BreachAtFirstBlockLine {
        controller: TrafficController,
        client: IpAddr,
        breached: std::sync::atomic::AtomicBool,
    }

    impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for BreachAtFirstBlockLine {
        fn on_event(
            &self,
            event: &tracing::Event<'_>,
            _: tracing_subscriber::layer::Context<'_, S>,
        ) {
            if event.metadata().target() == block_log::BLOCK_LOG_TARGET
                && !self
                    .breached
                    .swap(true, std::sync::atomic::Ordering::Relaxed)
            {
                let tally = TrafficTally::new(Some(self.client), None, None, Weight::one());
                self.controller.tally(tally);
            }
        }
    }

    #[tokio::test]
    async fn a_client_blocked_as_dry_run_goes_off_gets_the_new_ttl() {
        let controller = TrafficController::init_for_test(policy_config(true, PolicyKind::Spam));
        // A block that dry run kept from the log, so that turning dry run off
        // logs its line.
        controller.tally(breach(PolicyKind::Spam));
        let subscriber = tracing_subscriber::registry().with(BreachAtFirstBlockLine {
            controller: controller.clone(),
            client: IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2)),
            breached: Default::default(),
        });
        tracing::subscriber::with_default(subscriber, || {
            controller
                .admin_reconfigure(TrafficControlReconfigParams {
                    dry_run: Some(false),
                    connection_blocklist_ttl_sec: Some(30),
                    ..Default::default()
                })
                .unwrap();
        });
        let blocklist = controller.blocklist(10);
        let block = blocklist
            .clients
            .iter()
            .find(|block| block.client == "10.0.0.2")
            .expect("the breach blocked the client");
        assert!(block.seconds_left <= 30, "{block:?}");
    }

    #[tokio::test]
    async fn turning_dry_run_off_and_the_default_deny_off_at_once_refuses_no_client() {
        let controller = TrafficController::init_for_test(PolicyConfig {
            access_rules: AccessRules {
                default: DefaultAction::Deny,
                ..Default::default()
            },
            ..policy_config(true, PolicyKind::Spam)
        });
        // A block that dry run kept from the log, so that turning dry run off
        // logs its line.
        controller.tally(breach(PolicyKind::Spam));
        let decisions = collect_decisions_at_events(
            &controller,
            block_log::BLOCK_LOG_TARGET,
            tracing::Level::INFO,
            || {
                controller
                    .admin_reconfigure(TrafficControlReconfigParams {
                        dry_run: Some(false),
                        default_action: Some(DefaultAction::Limit),
                        ..Default::default()
                    })
                    .unwrap();
            },
        );
        assert_eq!(decisions, [ClientAccess::Limited]);
    }

    #[tokio::test]
    async fn turning_dry_run_on_and_the_default_deny_on_at_once_refuses_no_client() {
        let controller = TrafficController::init_for_test(policy_config(false, PolicyKind::Spam));
        // The node warns once the default action deny is in force.
        let decisions = collect_decisions_at_events(
            &controller,
            "iota_traffic_controller",
            tracing::Level::WARN,
            || {
                controller
                    .admin_reconfigure(TrafficControlReconfigParams {
                        dry_run: Some(true),
                        default_action: Some(DefaultAction::Deny),
                        ..Default::default()
                    })
                    .unwrap();
            },
        );
        assert_eq!(decisions, [ClientAccess::Limited]);
    }

    #[tokio::test]
    async fn the_client_usage_lists_the_lowest_gas_balances_first() {
        let controller = gas_limited(false, vec![]);
        let low = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2));
        controller
            .reserve_simulation_gas(Some(CLIENT), ClientAccess::Limited)
            .unwrap()
            .unwrap()
            .settle(30, 1);
        controller
            .reserve_simulation_gas(Some(low), ClientAccess::Limited)
            .unwrap()
            .unwrap()
            .settle(60, 1);
        let usage = controller.client_usage(1);
        assert_eq!(usage.gas_balances.len(), 1);
        assert_eq!(usage.gas_balances[0].client.as_deref(), Some("10.0.0.2"));
        assert!((40..=41).contains(&usage.gas_balances[0].gas), "{usage:?}");
        assert_eq!(usage.omitted_gas_balances, 1);
    }

    fn denying_by_default() -> TrafficController {
        TrafficController::init_for_test(PolicyConfig {
            access_rules: AccessRules {
                allow: vec![CLIENT.to_string()],
                default: DefaultAction::Deny,
                ..Default::default()
            },
            dry_run: false,
            ..Default::default()
        })
    }

    #[tokio::test]
    async fn a_block_does_not_refuse_an_allowed_client() {
        let controller = TrafficController::init_for_test(policy_config(false, PolicyKind::Spam));
        controller.tally(breach(PolicyKind::Spam));
        assert_eq!(
            controller.check(&Some(CLIENT), &None),
            ClientAccess::Refused
        );

        // A block the client got before its rule was added does not refuse it.
        controller
            .add_access_rule(AccessRuleList::Allow, &CLIENT.to_string())
            .unwrap();
        assert_eq!(
            controller.check(&Some(CLIENT), &None),
            ClientAccess::Allowed
        );
    }

    #[tokio::test]
    async fn the_admin_api_turns_dry_run_on_for_the_default_deny() {
        let controller = denying_by_default();
        let stranger = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 5));
        assert!(!controller.check(&Some(stranger), &None).is_served());

        controller
            .admin_reconfigure(TrafficControlReconfigParams {
                error_threshold: None,
                spam_threshold: None,
                dry_run: Some(true),
                connection_blocklist_ttl_sec: None,
                proxy_blocklist_ttl_sec: None,
                ..Default::default()
            })
            .expect("the request changes only the dry-run flag");
        assert!(controller.check(&Some(stranger), &None).is_served());
    }

    #[tokio::test]
    async fn an_access_rule_added_at_runtime_applies_to_the_next_check() {
        let controller = denying_by_default();
        let stranger = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 5));
        assert!(
            controller
                .add_access_rule(AccessRuleList::Allow, "10.0.0.0/24")
                .unwrap()
        );
        assert_eq!(
            controller.check(&Some(stranger), &None),
            ClientAccess::Allowed
        );
        assert!(
            controller
                .remove_access_rule(AccessRuleList::Allow, "10.0.0.7/24")
                .unwrap()
        );
        assert_eq!(
            controller.check(&Some(stranger), &None),
            ClientAccess::Refused
        );
    }

    #[tokio::test]
    async fn the_admin_api_rejects_an_invalid_access_rule() {
        let controller = denying_by_default();
        assert!(matches!(
            controller.add_access_rule(AccessRuleList::Deny, "10.0.0.0/40"),
            Err(IotaError::InvalidAdminRequest(_))
        ));
    }

    #[tokio::test]
    async fn canonical_access_rules_are_the_rules_the_admin_api_reports() {
        let rules = AccessRules {
            allow: vec!["203.0.113.7".to_string(), "::ffff:10.0.0.0/104".to_string()],
            deny: vec!["2001:db8::1".to_string(), "198.51.100.7/24".to_string()],
            default: DefaultAction::Deny,
            forwarders: vec!["10.0.0.5/24".to_string()],
        };
        let controller = TrafficController::init_for_test(PolicyConfig {
            access_rules: rules.clone(),
            ..Default::default()
        });
        let canonical = canonical_access_rules(&rules).unwrap();
        assert_eq!(canonical, controller.access_rules());
        assert_eq!(canonical.allow, ["203.0.113.7/32", "10.0.0.0/8"]);
        assert_eq!(canonical.forwarders, ["10.0.0.0/24"]);
        let error = canonical_access_rules(&AccessRules {
            deny: vec!["10.0.0.300".to_string()],
            ..Default::default()
        })
        .unwrap_err();
        assert!(error.starts_with("access rule \"10.0.0.300\""), "{error}");
    }
}
