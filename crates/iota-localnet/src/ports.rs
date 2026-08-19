// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    io,
    net::{SocketAddr, TcpListener, UdpSocket},
};

use anyhow::bail;
use iota_config::NodeConfig;
use iota_swarm::memory::{Node, Swarm};

/// The transport an address is bound with.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Transport {
    Tcp,
    Udp,
}

const COMMITTEE_ADDRESS_ADVICE: &str =
    "it is fixed by the committee metadata in `genesis.blob`, so only a new genesis can move it";

/// An address a local network binds when it launches, named as the user sees
/// it and paired with the way to move it.
#[derive(Debug)]
pub struct BoundAddress {
    /// What binds the address, e.g. `validator-0 metrics-address`.
    bound_by: String,
    address: SocketAddr,
    transport: Transport,
    /// Whether only a new genesis moves the address.
    fixed_by_genesis: bool,
    /// What to do about the address when its port is taken.
    advice: String,
}

impl BoundAddress {
    /// An address a node config field holds, which `--node-config-override`
    /// can move.
    fn overridable(scope: &str, field: &str, address: SocketAddr, transport: Transport) -> Self {
        Self {
            bound_by: format!("{scope} {field}"),
            address,
            transport,
            fixed_by_genesis: false,
            advice: format!(
                "override it with --node-config-override {scope}:{field}={ip}:<port>",
                ip = address.ip()
            ),
        }
    }

    /// An address a new genesis moves and an override cannot; `advice` says
    /// which committee field pins it.
    fn fixed_by_genesis(
        scope: &str,
        field: &str,
        address: SocketAddr,
        transport: Transport,
        advice: &str,
    ) -> Self {
        Self {
            bound_by: format!("{scope} {field}"),
            address,
            transport,
            fixed_by_genesis: true,
            advice: advice.to_owned(),
        }
    }

    /// An address a service listens on, which `flag` moves.
    pub fn service(name: &str, flag: &str, address: SocketAddr) -> Self {
        Self {
            bound_by: name.to_owned(),
            address,
            transport: Transport::Tcp,
            fixed_by_genesis: false,
            advice: format!("move it with {flag}=<port>"),
        }
    }

    /// Whether something already holds the address. Any other reason a bind
    /// fails, such as an address this host does not have, is not a port clash
    /// and is left to the node to report.
    fn is_in_use(&self) -> bool {
        let bound = match self.transport {
            Transport::Tcp => TcpListener::bind(self.address).map(drop),
            Transport::Udp => UdpSocket::bind(self.address).map(drop),
        };
        matches!(bound, Err(err) if err.kind() == io::ErrorKind::AddrInUse)
    }

    /// Whether both addresses want the same socket, so that whichever binds
    /// first takes it from the other. A wildcard address covers every IP of
    /// the host, so it clashes with any address on its port.
    fn clashes_with(&self, other: &Self) -> bool {
        self.transport == other.transport
            && self.address.port() == other.address.port()
            && (self.address.ip() == other.address.ip()
                || self.address.ip().is_unspecified()
                || other.address.ip().is_unspecified())
    }
}

/// Fail if two of `addresses` want one socket, or if something else already
/// holds one of them, naming every clash and what to do about it.
///
/// This runs just before the network launches and reserves nothing: each port
/// is free again the moment it has been probed, so one that passes here can
/// still be taken by the time a node binds it. It turns the common case — a
/// second local network, another program on a fixed port, or one port given
/// to two things of this run — into a message naming the node and the field,
/// instead of a bind failure from inside a node.
pub fn check_ports_are_free(addresses: &[BoundAddress]) -> Result<(), anyhow::Error> {
    let mut clashes = Vec::new();

    for (index, address) in addresses.iter().enumerate() {
        // Only the first later address on the port is reported, so that a port
        // several of them want reads as a chain of pairs rather than as every
        // combination of them.
        let Some(other) = addresses[index + 1..]
            .iter()
            .find(|other| address.clashes_with(other))
        else {
            continue;
        };
        // Only a new genesis moves a genesis-fixed address, so advise on the
        // other one wherever there is a choice.
        let advice = if other.fixed_by_genesis && !address.fixed_by_genesis {
            &address.advice
        } else {
            &other.advice
        };
        clashes.push(format!(
            "port {port} is bound twice by this run ({bound_by}, {other_bound_by})\n  {advice}",
            port = address.address.port(),
            bound_by = address.bound_by,
            other_bound_by = other.bound_by,
        ));
    }

    clashes.extend(
        addresses
            .iter()
            .filter(|address| address.is_in_use())
            .map(|address| {
                format!(
                    "port {port} ({bound_by}) is already in use\n  {advice}",
                    port = address.address.port(),
                    bound_by = address.bound_by,
                    advice = address.advice
                )
            }),
    );

    if clashes.is_empty() {
        return Ok(());
    }
    bail!(clashes.join("\n"))
}

/// The addresses the nodes of `swarm` bind when the network launches.
///
/// A node binds fewer addresses than its config names: `iota-localnet` runs
/// the nodes in-process, which starts no admin interface, and a validator
/// serves neither the JSON-RPC nor the gRPC API.
pub fn addresses_bound_at_launch(swarm: &Swarm) -> Vec<BoundAddress> {
    let mut addresses = Vec::new();

    let committee = swarm.config().committee_with_network();
    for (index, config) in swarm.config().validator_configs().iter().enumerate() {
        // A validator's consensus address has no node config field of its
        // own: it reads its own, like its peers', from the committee.
        let primary_address = committee
            .validators()
            .get(&config.authority_public_key())
            .and_then(|(_, metadata)| metadata.primary_address.udp_multiaddr_to_listen_address());
        addresses.extend(validator_addresses(
            &format!("validator-{index}"),
            config,
            primary_address,
        ));
    }

    // The swarm keeps its nodes in a map, so sort them to report the same
    // order on every run.
    let mut fullnodes: Vec<&Node> = swarm.fullnodes().collect();
    fullnodes.sort_by_key(|fullnode| fullnode.name());
    for fullnode in fullnodes {
        addresses.extend(fullnode_addresses(&fullnode.config()));
    }

    addresses
}

/// The addresses a validator binds.
fn validator_addresses(
    scope: &str,
    config: &NodeConfig,
    primary_address: Option<SocketAddr>,
) -> Vec<BoundAddress> {
    let mut addresses = Vec::new();

    if let Ok(network_address) = config.network_address.to_socket_addr() {
        addresses.push(BoundAddress::fixed_by_genesis(
            scope,
            "network-address",
            network_address,
            Transport::Tcp,
            COMMITTEE_ADDRESS_ADVICE,
        ));
    }
    // The listen address itself is overridable, but its port is not.
    addresses.push(BoundAddress::fixed_by_genesis(
        scope,
        "p2p-config.listen-address",
        config.p2p_config.listen_address,
        Transport::Udp,
        "its port is the validator's `p2p-address` in the committee metadata of `genesis.blob`, \
         so only a new genesis can move it",
    ));
    addresses.push(BoundAddress::overridable(
        scope,
        "metrics-address",
        config.metrics_address,
        Transport::Tcp,
    ));
    if let Some(primary_address) = primary_address {
        // Consensus serves this over TCP, even though the committee writes it
        // as a UDP multiaddr.
        addresses.push(BoundAddress::fixed_by_genesis(
            scope,
            "primary-address",
            primary_address,
            Transport::Tcp,
            COMMITTEE_ADDRESS_ADVICE,
        ));
    }

    addresses
}

/// The addresses a fullnode binds.
fn fullnode_addresses(config: &NodeConfig) -> Vec<BoundAddress> {
    let mut addresses = vec![
        BoundAddress::overridable(
            "fullnode",
            "json-rpc-address",
            config.json_rpc_address,
            Transport::Tcp,
        ),
        BoundAddress::overridable(
            "fullnode",
            "metrics-address",
            config.metrics_address,
            Transport::Tcp,
        ),
        BoundAddress::overridable(
            "fullnode",
            "p2p-config.listen-address",
            config.p2p_config.listen_address,
            Transport::Udp,
        ),
    ];

    if config.enable_grpc_api {
        if let Some(grpc_api_config) = &config.grpc_api_config {
            addresses.push(BoundAddress::overridable(
                "fullnode",
                "grpc-api-config.address",
                grpc_api_config.address,
                Transport::Tcp,
            ));
        }
    }

    addresses
}

#[cfg(test)]
mod tests {
    use std::{net::Ipv4Addr, path::Path};

    use iota_config::node::Genesis;
    use iota_swarm_config::{
        genesis_config::ValidatorGenesisConfigBuilder,
        node_config_builder::{FullnodeConfigBuilder, ValidatorConfigBuilder},
    };
    use rand::rngs::OsRng;

    use super::*;

    /// A validator on the localnet port layout: its node config, and the
    /// primary address its committee entry carries, both derived from one
    /// genesis entry as `start` derives them.
    ///
    /// A test that probes these addresses passes a `port_base` away from the
    /// real one, so that a local network running beside the test does not read
    /// as a clash, and its own, so that two tests binding at the same moment
    /// do not read as one.
    fn validator_config(directory: &Path, port_base: u16) -> (NodeConfig, Option<SocketAddr>) {
        let validator = ValidatorGenesisConfigBuilder::new()
            .with_ip("127.0.0.1".to_owned())
            .with_deterministic_ports(port_base)
            .build(&mut OsRng);
        let primary_address = validator.primary_address.udp_multiaddr_to_listen_address();
        let config = ValidatorConfigBuilder::new()
            .with_config_directory(directory.to_path_buf())
            .build_without_genesis(validator);
        (config, primary_address)
    }

    fn fullnode_config(directory: &Path) -> NodeConfig {
        FullnodeConfigBuilder::new()
            .with_config_directory(directory.to_path_buf())
            .build_from_parts(&mut OsRng, &[], Genesis::new_empty())
    }

    /// Hold a TCP port for as long as the returned listener lives.
    fn occupy_tcp_port() -> (TcpListener, u16) {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        (listener, port)
    }

    /// Hold a UDP port for as long as the returned socket lives.
    fn occupy_udp_port() -> (UdpSocket, u16) {
        let socket = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = socket.local_addr().unwrap().port();
        (socket, port)
    }

    #[test]
    fn a_committee_address_is_reported_as_one_only_a_new_genesis_moves() {
        let directory = tempfile::tempdir().unwrap();
        let (_socket, port) = occupy_udp_port();
        let (mut config, primary_address) = validator_config(directory.path(), 29210);
        config.p2p_config.listen_address = (Ipv4Addr::LOCALHOST, port).into();

        let addresses = validator_addresses("validator-0", &config, primary_address);
        let err = check_ports_are_free(&addresses).unwrap_err().to_string();

        assert_eq!(
            err,
            format!(
                "port {port} (validator-0 p2p-config.listen-address) is already in use\n  its \
                 port is the validator's `p2p-address` in the committee metadata of \
                 `genesis.blob`, so only a new genesis can move it"
            )
        );
        assert!(!err.contains("--node-config-override"), "{err}");
    }

    #[test]
    fn every_clash_is_reported_not_only_the_first() {
        let directory = tempfile::tempdir().unwrap();
        let (_metrics_listener, metrics_port) = occupy_tcp_port();
        let (_json_rpc_listener, json_rpc_port) = occupy_tcp_port();
        let (mut validator_config, primary_address) = validator_config(directory.path(), 29220);
        validator_config.metrics_address = (Ipv4Addr::LOCALHOST, metrics_port).into();
        let mut fullnode_config = fullnode_config(directory.path());
        fullnode_config.json_rpc_address = (Ipv4Addr::LOCALHOST, json_rpc_port).into();

        let mut addresses = validator_addresses("validator-0", &validator_config, primary_address);
        addresses.extend(fullnode_addresses(&fullnode_config));
        let err = check_ports_are_free(&addresses).unwrap_err().to_string();

        assert_eq!(
            err,
            format!(
                "port {metrics_port} (validator-0 metrics-address) is already in use\n  override \
                 it with --node-config-override validator-0:metrics-address=127.0.0.1:<port>\n\
                 port {json_rpc_port} (fullnode json-rpc-address) is already in use\n  override \
                 it with --node-config-override fullnode:json-rpc-address=127.0.0.1:<port>"
            )
        );
    }

    #[test]
    fn a_service_clash_names_the_flag_that_moves_it() {
        let (_listener, port) = occupy_tcp_port();
        let addresses = [BoundAddress::service(
            "faucet",
            "--with-faucet",
            (Ipv4Addr::LOCALHOST, port).into(),
        )];

        let err = check_ports_are_free(&addresses).unwrap_err().to_string();

        assert_eq!(
            err,
            format!("port {port} (faucet) is already in use\n  move it with --with-faucet=<port>")
        );
    }

    /// Two addresses of one run on one port never both bind, however free the
    /// port is, so the check has to compare the run's own addresses too.
    #[test]
    fn a_port_this_run_binds_twice_is_reported() {
        // TEST-NET-1, which is not assigned to an interface, so that nothing
        // but the comparison between these addresses can report them.
        let address: SocketAddr = "192.0.2.1:9123".parse().unwrap();
        let addresses = [
            BoundAddress::overridable("fullnode", "json-rpc-address", address, Transport::Tcp),
            BoundAddress::service("faucet", "--with-faucet", address),
            // The same port over the other transport is another socket.
            BoundAddress::overridable(
                "fullnode",
                "p2p-config.listen-address",
                address,
                Transport::Udp,
            ),
        ];

        let err = check_ports_are_free(&addresses).unwrap_err().to_string();

        assert_eq!(
            err,
            "port 9123 is bound twice by this run (fullnode json-rpc-address, faucet)\n  move it \
             with --with-faucet=<port>"
        );
    }

    /// Only a port something else holds is a clash. A bind that fails for
    /// another reason must not be reported, or every address this host does
    /// not have would look occupied.
    #[test]
    fn an_address_this_host_does_not_have_is_not_a_clash() {
        // TEST-NET-1, which is not assigned to an interface.
        let addresses = [BoundAddress::service(
            "faucet",
            "--with-faucet",
            "192.0.2.1:9123".parse().unwrap(),
        )];

        check_ports_are_free(&addresses).unwrap();
    }

    /// A validator serves no JSON-RPC, and nothing starts its admin interface
    /// in-process, so neither address is checked.
    #[test]
    fn a_validator_binds_neither_its_json_rpc_nor_its_admin_interface() {
        let directory = tempfile::tempdir().unwrap();
        let (config, primary_address) = validator_config(directory.path(), 9200);

        let addresses = validator_addresses("validator-0", &config, primary_address);

        let bound_by: Vec<&str> = addresses
            .iter()
            .map(|address| address.bound_by.as_str())
            .collect();
        assert_eq!(
            bound_by,
            [
                "validator-0 network-address",
                "validator-0 p2p-config.listen-address",
                "validator-0 metrics-address",
                "validator-0 primary-address",
            ]
        );
        let ports: Vec<u16> = addresses
            .iter()
            .map(|address| address.address.port())
            .collect();
        assert_eq!(ports, [9200, 9201, 9202, 9203]);
    }

    /// The gRPC API is off unless a run asks for it, and an address nothing
    /// binds must not be reported as taken.
    #[test]
    fn a_fullnode_binds_its_grpc_api_address_only_when_the_api_is_on() {
        let directory = tempfile::tempdir().unwrap();
        let mut config = fullnode_config(directory.path());

        config.enable_grpc_api = false;
        assert!(
            !fullnode_addresses(&config)
                .iter()
                .any(|address| address.bound_by.contains("grpc-api-config.address"))
        );

        config.enable_grpc_api = true;
        config.grpc_api_config = Some(Default::default());
        assert!(
            fullnode_addresses(&config)
                .iter()
                .any(|address| address.bound_by.contains("grpc-api-config.address"))
        );
    }
}
