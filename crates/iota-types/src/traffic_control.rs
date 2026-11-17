// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use rand::distr::Distribution;
use serde::{Deserialize, Serialize, de::Deserializer};
use serde_with::serde_as;

/// The source that should be used to identify the client's
/// IP address. To be used to configure cases where a node has
/// infra running in front of the node that is separate from the
/// protocol, such as a load balancer. Note that this is not the
/// same as the client type (e.g a direct client vs a proxy client,
/// as in the case of a fullnode driving requests from many clients).
///
/// For x-forwarded-for, the usize parameter is the number of forwarding
/// hops between the client and the node for requests going your infra
/// or infra provider. Example:
///
/// ```ignore
///     (client) -> { (global proxy) -> (regional proxy) -> (node) }
/// ```
///
/// where
///
/// ```ignore
///     { <server>, ... }
/// ```
///
/// are controlled by the Node operator / their cloud provider.
/// In this case, we set:
///
/// ```ignore
/// policy-config:
///    client-id-source:
///      x-forwarded-for: 2
///    ...
/// ```
///
/// NOTE: x-forwarded-for: 0 is a special case value that can be used by Node
/// operators to discover the number of hops that should be configured. To use:
///
/// 1. Set `x-forwarded-for: 0` for the `client-id-source` in the config.
/// 2. Run the node and query any endpoint (AuthorityServer for validator, or json rpc for rpc node)
///    from a known IP address.
/// 3. Search for lines containing `x-forwarded-for` in the logs. The log lines should contain the
///    contents of the `x-forwarded-for` header, if present, or a corresponding error if not.
/// 4. The value for number of hops is derived from any such log line that contains your known IP
///    address, and is defined as 1 + the number of IP addresses in the `x-forwarded-for` that occur
///    **after** the known client IP address. Example:
///
/// ```ignore
///     [<known client IP>] <--- number of hops is 1
///     ["1.2.3.4", <known client IP>, "5.6.7.8", "9.10.11.12"] <--- number of hops is 3
/// ```
#[derive(Clone, Debug, Deserialize, Serialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ClientIdSource {
    #[default]
    SocketAddr,
    XForwardedFor(usize),
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct TrafficControlReconfigParams {
    pub error_threshold: Option<u64>,
    pub spam_threshold: Option<u64>,
    pub dry_run: Option<bool>,
    pub connection_blocklist_ttl_sec: Option<u64>,
    pub proxy_blocklist_ttl_sec: Option<u64>,
    /// Sets `gas-per-sec` of the simulation gas limit. Turning the limit on
    /// takes this and `simulation_burst_gas` together.
    pub simulation_gas_per_sec: Option<u64>,
    /// Sets `burst-gas` of the simulation gas limit.
    pub simulation_burst_gas: Option<u64>,
    /// Sets the default action of the access rules. Outside dry run, `deny`
    /// refuses every client that no allow rule names. On a validator this also
    /// refuses the other nodes that send it requests.
    pub default_action: Option<DefaultAction>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Weight(f32);

impl Weight {
    pub fn new(value: f32) -> Result<Self, &'static str> {
        if (0.0..=1.0).contains(&value) {
            Ok(Self(value))
        } else {
            Err("Weight must be between 0.0 and 1.0")
        }
    }

    pub fn one() -> Self {
        Self(1.0)
    }

    pub fn zero() -> Self {
        Self(0.0)
    }

    pub fn value(&self) -> f32 {
        self.0
    }

    pub fn is_sampled(&self) -> bool {
        let mut rng = rand::rng();
        // `Uniform::new` excludes the upper bound, so a weight of 1.0 accepts every
        // sample.
        let sample = rand::distr::Uniform::new(0.0, 1.0)
            .unwrap()
            .sample(&mut rng);
        self.accepts(sample)
    }

    fn accepts(&self, sample: f32) -> bool {
        sample < self.value()
    }
}

fn validate_sample_rate<'de, D>(deserializer: D) -> Result<Weight, D::Error>
where
    D: Deserializer<'de>,
{
    let value = f32::deserialize(deserializer)?;
    Weight::new(value)
        .map_err(|_| serde::de::Error::custom("spam-sample-rate must be between 0.0 and 1.0"))
}

impl PartialEq for Weight {
    fn eq(&self, other: &Self) -> bool {
        self.value() == other.value()
    }
}

#[serde_as]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct FreqThresholdConfig {
    /// Sustained tallies per second allowed from a single direct client. Zero
    /// blocks every client; values above 10^9 are clamped.
    #[serde(default = "default_client_threshold")]
    pub client_threshold: u64,
    /// Sustained tallies per second allowed from a single proxied client.
    #[serde(default = "default_proxied_client_threshold")]
    pub proxied_client_threshold: u64,
    /// The burst allowance, in seconds of the sustained rate. A client can
    /// send `threshold * burst_secs` tallies back to back before the node
    /// blocks it. The node limits the burst to `u32::MAX` tallies. A value of
    /// zero stops the node at startup. The former name `window-size-secs` is
    /// deprecated, but the node continues to accept it.
    #[serde(default = "default_burst_secs", alias = "window-size-secs")]
    pub burst_secs: u64,
}

impl Default for FreqThresholdConfig {
    fn default() -> Self {
        Self {
            client_threshold: default_client_threshold(),
            proxied_client_threshold: default_proxied_client_threshold(),
            burst_secs: default_burst_secs(),
        }
    }
}

fn default_client_threshold() -> u64 {
    // By default only block clients with an unreasonably high sustained rate,
    // as a client could be a single fullnode proxying the traffic of many
    // behaving clients. In practice this should always be set.
    1_000_000
}

fn default_proxied_client_threshold() -> u64 {
    10
}

fn default_burst_secs() -> u64 {
    30
}

// Serializable representation of policy types, used in config
// in order to easily change in tests or to killswitch
#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub enum PolicyType {
    /// Does nothing
    #[default]
    NoOp,

    /// Rate limits each client IP to `threshold` tallies per second, tolerating
    /// a burst of `threshold * burst_secs` tallies.
    #[serde(rename = "freq-threshold", alias = "FreqThreshold")]
    FreqThreshold(FreqThresholdConfig),

    // Below this point are test policies, and thus should not be used in production
    /// Simple policy that adds connection_ip to blocklist when the same
    /// connection_ip is encountered in tally N times. If used in an error
    /// policy, this would trigger after N errors
    TestNConnIP(u64),
    /// Test policy that panics when invoked. To be used as an error policy in
    /// tests that do not expect request errors in order to verify that the
    /// error policy is not invoked
    TestPanicOnInvocation,
}

/// Why the node refuses a client on its blocklist.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BlockReason {
    /// The client exceeded the limit of the spam policy.
    Spam,
    /// The client exceeded the limit of the error policy.
    Error,
    /// An operator blocked the client through the admin API.
    Manual,
}

impl std::fmt::Display for BlockReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Spam => "spam",
            Self::Error => "error",
            Self::Manual => "manual",
        })
    }
}

/// The traffic control settings in force, as the admin API reports them.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct TrafficControlStatus {
    pub error_threshold: Option<u64>,
    pub spam_threshold: Option<u64>,
    pub dry_run: bool,
    pub connection_blocklist_ttl_sec: u64,
    pub proxy_blocklist_ttl_sec: u64,
    pub default_action: DefaultAction,
    pub allow_rules: usize,
    pub deny_rules: usize,
    /// The clients with a block in force, as direct clients, as clients that
    /// reached the node through a fullnode, or as both.
    pub blocked_clients: usize,
    pub simulation_gas_limit: Option<SimulationGasLimitConfig>,
}

/// A client on the blocklist. `client` is an IP address, or an IPv6 prefix
/// such as `2001:db8:1:2::/64`, as the block log line names it.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct BlockedClient {
    pub client: String,
    /// Whether the client reached the node through a fullnode.
    pub proxied: bool,
    pub reason: BlockReason,
    pub seconds_left: u64,
}

/// Part of the blocklist: `omitted` clients were left out to keep the list
/// within its limit.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct BlockedClients {
    pub clients: Vec<BlockedClient>,
    pub omitted: usize,
}

/// The simulation gas balance of a client that has not got all of its
/// `burst-gas`. `client` is `None` for the clients whose IP the node could
/// not resolve, which share one balance.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct ClientGasBalance {
    pub client: Option<String>,
    pub gas: u64,
}

/// The clients with the lowest simulation gas balances, lowest first, and the
/// clients on the blocklist. `omitted_*` count those left out.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct ClientUsage {
    pub gas_balances: Vec<ClientGasBalance>,
    pub omitted_gas_balances: usize,
    pub blocked: BlockedClients,
}

/// The gas each client may spend on simulations, which run transactions for
/// free: gRPC `SimulateTransactions` and `ViewFunctionCalls`, and JSON-RPC
/// `iota_dryRunTransactionBlock`, `iota_devInspectTransactionBlock` and
/// `iota_view`. Each client has a gas balance that refills by `gas-per-sec` up
/// to `burst-gas`. A simulation is refused while the balance of its client is
/// below the computation cap of one transaction (`max_gas_computation_bucket`
/// of the protocol), and the client's other requests are still served. Both
/// values are in gas units and must be above 0.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct SimulationGasLimitConfig {
    /// How much gas a client's balance refills by each second.
    pub gas_per_sec: u64,
    /// The largest balance a client can save up.
    pub burst_gas: u64,
}

/// One of the two lists of access rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AccessRuleList {
    Allow,
    Deny,
}

/// What the node does with a client that no access rule names and the
/// blocklist does not hold.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DefaultAction {
    /// Serve the client, and let the policies count it and block it.
    #[default]
    Limit,
    /// Refuse the client.
    Deny,
}

/// The access rules of a node, checked in this order, where the first match
/// decides:
///
/// 1. `allow`: the node serves the client, and no policy counts or blocks it.
/// 2. `deny`: the node refuses the client.
/// 3. The blocklist: the node refuses a client that a policy blocked, until the block expires.
/// 4. `default`: what the node does with every other client.
///
/// A rule is an IP address, or a network in CIDR notation such as
/// `198.51.100.0/24`, and matches the full address of the client that connects
/// to the node, or of the client that `client-id-source` names. An IPv4 client
/// is matched by its IPv4 address, also when it connects as an IPv4-mapped
/// IPv6 address, so IPv4 rules match it, and IPv6 rules do not, other than an
/// IPv4-mapped rule of `/96` or longer, which is read as the IPv4 rule. Under
/// dry run the node serves the clients that steps 2 to 4 refuse, and counts
/// them.
///
/// `forwarders` names the services, such as GraphQL and the indexer, that
/// call a fullnode for their own users and name each user in the
/// [`FORWARDED_CLIENT_IP_HEADER`] header. The fullnode then takes that user as
/// the client of the request, for the rules and every policy, and does not
/// count the forwarder. The header is ignored from any other client, and a
/// forwarder's request without exactly one such header holding an IP address
/// is the forwarder's own. Validators do not apply `forwarders`.
///
/// ```yaml
/// access-rules:
///   allow:
///     - 203.0.113.7
///     - 198.51.100.0/24
///   deny:
///     - 192.0.2.0/24
///   default: limit
///   forwarders:
///     - 10.0.0.5
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct AccessRules {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allow: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deny: Vec<String>,
    #[serde(default)]
    pub default: DefaultAction,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub forwarders: Vec<String>,
}

/// The header in which a forwarder (see [`AccessRules`]) names the user it
/// calls the node for, as an IP address.
pub const FORWARDED_CLIENT_IP_HEADER: &str = "x-iota-client-ip";

impl AccessRules {
    /// Whether these rules do the same as no `access-rules` in the config.
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// The error a config that sets `allow-list` fails to load with.
const ALLOW_LIST_REMOVED: &str = "`allow-list` is no longer supported. To serve only the \
    listed clients, use `access-rules` instead:\n\n  access-rules:\n    allow:\n      \
    - <IP address or CIDR network>\n    default: deny";

/// Accepts `allow-list` only when it is `null`, which older versions wrote
/// for a config without an allowlist.
fn reject_allow_list<'de, D>(deserializer: D) -> Result<Option<Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    match Option::<Vec<String>>::deserialize(deserializer)? {
        None => Ok(None),
        Some(_) => Err(serde::de::Error::custom(ALLOW_LIST_REMOVED)),
    }
}

#[serde_as]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct PolicyConfig {
    #[serde(default = "default_client_id_source")]
    pub client_id_source: ClientIdSource,
    #[serde(default = "default_blocklist_ttl_sec")]
    pub connection_blocklist_ttl_sec: u64,
    #[serde(default = "default_blocklist_ttl_sec")]
    pub proxy_blocklist_ttl_sec: u64,
    #[serde(default)]
    pub spam_policy_type: PolicyType,
    #[serde(default)]
    pub error_policy_type: PolicyType,
    #[serde(
        default = "default_spam_sample_rate",
        deserialize_with = "validate_sample_rate"
    )]
    /// Note that this sample policy is applied on top of the
    /// endpoint-specific sample policy (not configurable) which
    /// weighs endpoints by the relative effort required to serve
    /// them. Therefore a sample rate of N will yield an actual
    /// sample rate <= N.
    pub spam_sample_rate: Weight,
    #[serde(default = "default_dry_run")]
    pub dry_run: bool,
    /// No longer accepted: a config that sets `allow-list` to a list fails to
    /// load, with an error that shows the `access-rules` that replace it.
    #[serde(
        default,
        deserialize_with = "reject_allow_list",
        skip_serializing_if = "Option::is_none"
    )]
    pub allow_list: Option<Vec<String>>,
    /// Which clients the node serves without counting them, which it refuses,
    /// and what it does with every other client.
    #[serde(default, skip_serializing_if = "AccessRules::is_default")]
    pub access_rules: AccessRules,
    /// The gas each client may spend on simulations. Absent, simulations have
    /// no gas limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub simulation_gas_limit: Option<SimulationGasLimitConfig>,
    /// How many leading bits of an IPv6 address identify a client, from 32 to
    /// 128. The node cannot see how long a client's own network prefix is, so
    /// it assumes this length for every IPv6 client: the policies count, and
    /// the node blocks, the addresses that share these bits as one client.
    /// One subscriber usually holds a whole /64 and can switch addresses
    /// within it. 128 treats every address as a client of its own. An IPv4
    /// client is always its full address.
    #[serde(default = "default_ipv6_prefix_len")]
    pub ipv6_prefix_len: u8,
}

impl Default for PolicyConfig {
    fn default() -> Self {
        Self {
            client_id_source: default_client_id_source(),
            connection_blocklist_ttl_sec: default_blocklist_ttl_sec(),
            proxy_blocklist_ttl_sec: default_blocklist_ttl_sec(),
            spam_policy_type: PolicyType::NoOp,
            error_policy_type: PolicyType::NoOp,
            spam_sample_rate: default_spam_sample_rate(),
            dry_run: default_dry_run(),
            allow_list: None,
            access_rules: AccessRules::default(),
            simulation_gas_limit: None,
            ipv6_prefix_len: default_ipv6_prefix_len(),
        }
    }
}

impl PolicyConfig {
    pub fn default_dos_protection_policy() -> Self {
        Self {
            client_id_source: ClientIdSource::SocketAddr,
            spam_policy_type: PolicyType::FreqThreshold(FreqThresholdConfig {
                client_threshold: 1000,
                burst_secs: 5,
                ..FreqThresholdConfig::default()
            }),
            error_policy_type: PolicyType::FreqThreshold(FreqThresholdConfig {
                client_threshold: 50,
                burst_secs: 5,
                ..FreqThresholdConfig::default()
            }),
            spam_sample_rate: Weight::new(1.0).unwrap(),
            dry_run: true,
            ..Self::default()
        }
    }
}

pub fn default_client_id_source() -> ClientIdSource {
    ClientIdSource::SocketAddr
}

pub fn default_blocklist_ttl_sec() -> u64 {
    60
}
pub fn default_ipv6_prefix_len() -> u8 {
    64
}

pub fn default_dry_run() -> bool {
    true
}

pub fn default_spam_sample_rate() -> Weight {
    Weight::new(0.2).unwrap()
}

#[cfg(test)]
mod tests {
    use super::Weight;

    #[test]
    fn zero_weight_rejects_the_lowest_sample() {
        assert!(!Weight::zero().accepts(0.0));
    }

    #[test]
    fn full_weight_accepts_the_highest_sample() {
        assert!(Weight::one().accepts(1.0 - f32::EPSILON));
    }

    #[test]
    fn the_default_dos_protection_policy_blocks_for_a_nonzero_time() {
        // A TTL of zero would expire every block at once, thus the policy would
        // never keep a client out even with dry run off.
        let policy = super::PolicyConfig::default_dos_protection_policy();
        assert!(policy.connection_blocklist_ttl_sec > 0);
        assert!(policy.proxy_blocklist_ttl_sec > 0);
    }

    #[test]
    fn a_config_with_an_allow_list_fails_to_load_and_names_the_replacement() {
        let error = serde_json::from_str::<super::PolicyConfig>(r#"{"allow-list": ["10.0.0.1"]}"#)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("`allow-list` is no longer supported"),
            "{error}"
        );
        assert!(error.contains("access-rules"), "{error}");
        assert!(error.contains("default: deny"), "{error}");
    }

    /// Older versions wrote `allow-list: null` for every config without an
    /// allowlist, and such a config still loads.
    #[test]
    fn a_config_with_a_null_allow_list_loads() {
        let config: super::PolicyConfig = serde_json::from_str(r#"{"allow-list": null}"#).unwrap();
        assert_eq!(config.allow_list, None);
        assert!(config.access_rules.is_default());
    }

    #[test]
    fn an_unknown_access_rules_key_fails_to_load() {
        let error =
            serde_json::from_str::<super::PolicyConfig>(r#"{"access-rules": {"defualt": "deny"}}"#)
                .unwrap_err()
                .to_string();
        assert!(error.contains("unknown field `defualt`"), "{error}");
    }

    #[test]
    fn access_rules_parse_with_a_default_of_limit() {
        let config: super::PolicyConfig =
            serde_json::from_str(r#"{"access-rules": {"allow": ["198.51.100.0/24"]}}"#).unwrap();
        assert_eq!(config.access_rules.allow, ["198.51.100.0/24"]);
        assert_eq!(config.access_rules.default, super::DefaultAction::Limit);

        let config: super::PolicyConfig =
            serde_json::from_str(r#"{"access-rules": {"default": "deny"}}"#).unwrap();
        assert_eq!(config.access_rules.default, super::DefaultAction::Deny);
    }

    #[test]
    fn the_former_burst_key_still_parses() {
        let config: super::FreqThresholdConfig =
            serde_json::from_str(r#"{"window-size-secs": 7}"#).unwrap();
        assert_eq!(config.burst_secs, 7);
    }
}
