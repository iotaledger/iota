// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    fs,
    net::{AddrParseError, IpAddr, Ipv4Addr, SocketAddr},
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, anyhow, bail, ensure};
use clap::*;
use colored::Colorize;
use fastcrypto::traits::KeyPair;
use iota_config::{
    Config, IOTA_BENCHMARK_GENESIS_GAS_KEYSTORE_FILENAME, IOTA_CLIENT_CONFIG, IOTA_FULLNODE_CONFIG,
    IOTA_GENESIS_FILENAME, IOTA_KEYSTORE_FILENAME, IOTA_NETWORK_CONFIG, NodeConfig,
    PersistedConfig, genesis_blob_exists, iota_config_dir, local_ip_utils,
    node::{Genesis, GrpcApiConfig},
    p2p::SeedPeer,
};
use iota_faucet::{AppState, FaucetConfig, SimpleFaucet, create_wallet_context, start_faucet};
#[cfg(feature = "indexer")]
use iota_graphql_rpc::{
    config::ConnectionConfig, test_infra::cluster::start_graphql_server_with_fn_rpc,
};
#[cfg(feature = "indexer")]
use iota_indexer::{
    config::PruningOptions,
    test_utils::{IndexerTypeConfig, start_test_indexer},
};
use iota_keys::keystore::{AccountKeystore, FileBasedKeystore, Keystore};
use iota_sdk::iota_client_config::{IotaClientConfig, IotaEnv};
use iota_sdk_crypto::simple::SimpleKeypair;
use iota_sdk_types::Address;
use iota_swarm::memory::{Node, Swarm};
use iota_swarm_config::{
    genesis_config::{
        GenesisConfig, SsfnGenesisConfig, ValidatorGenesisConfig, ValidatorGenesisConfigBuilder,
    },
    network_config::{NetworkConfig, PersistedNetworkConfig},
    network_config_builder::ConfigBuilder,
    node_config_builder::FullnodeConfigBuilder,
    node_config_override::{
        NodeConfigOverride, OverrideScope, overrides_for_fullnode, overrides_for_validator,
        winning_field_paths,
    },
};
use iota_types::traffic_control::PolicyConfig;
use rand::rngs::OsRng;
use tempfile::tempdir;
use tracing::info;

const CONCURRENCY_LIMIT: usize = 30;
const DEFAULT_COMMITTEE_SIZE: usize = 1;
const DEFAULT_EPOCH_DURATION_MS: u64 = 60_000;
const DEFAULT_FAUCET_NUM_COINS: usize = 5;
const DEFAULT_FAUCET_NANOS_AMOUNT: u64 = 200_000_000_000; // 200 IOTA
const DEFAULT_FAUCET_PORT: u16 = 9123;
const DEFAULT_GRPC_PORT: u16 = 50051;
#[cfg(feature = "indexer")]
const DEFAULT_GRAPHQL_PORT: u16 = 9125;
/// Metrics port a local network gives the GraphQL service, overriding the
/// `9184` the service picks by default, which the fullnode metrics endpoint
/// already holds. Metrics endpoints stay on `127.0.0.1`, as the node ones do.
#[cfg(feature = "indexer")]
const DEFAULT_GRAPHQL_METRICS_PORT: u16 = 9126;
#[cfg(feature = "indexer")]
const DEFAULT_INDEXER_PORT: u16 = 9124;
/// Port base of the fullnode layout, see [`fullnode_genesis_config`].
const FULLNODE_PORT_BASE: u16 = 9184;
/// Port base of the validator layout, see
/// [`ConfigBuilder::with_deterministic_ports`].
const VALIDATOR_PORT_BASE: u16 = 9200;

#[cfg(feature = "indexer")]
#[derive(Args)]
pub struct IndexerFeatureArgs {
    /// Start an indexer with default host and port: 0.0.0.0:9124. This flag
    /// accepts also a port, a host, or both (e.g., 0.0.0.0:9124).
    /// When providing a specific value, please use the = sign between the flag
    /// and value: `--with-indexer=6124` or `--with-indexer=0.0.0.0`, or
    /// `--with-indexer=0.0.0.0:9124` The indexer will be started in writer
    /// mode and reader mode.
    #[arg(long,
            default_missing_value = "0.0.0.0:9124",
            num_args = 0..=1,
            require_equals = true,
            value_name = "INDEXER_HOST_PORT",
        )]
    with_indexer: Option<String>,
    /// Start a GraphQL server with default host and port: 0.0.0.0:9125. This
    /// flag accepts also a port, a host, or both (e.g., 0.0.0.0:9125).
    /// When providing a specific value, please use the = sign between the flag
    /// and value: `--with-graphql=6124` or `--with-graphql=0.0.0.0`, or
    /// `--with-graphql=0.0.0.0:9125` Note that GraphQL requires a running
    /// indexer, which will be enabled by default if the `--with-indexer`
    /// flag is not set.
    #[arg(
            long,
            default_missing_value = "0.0.0.0:9125",
            num_args = 0..=1,
            require_equals = true,
            value_name = "GRAPHQL_HOST_PORT"
        )]
    with_graphql: Option<String>,
    /// Bind the GraphQL metrics endpoint to this host and port instead of
    /// 127.0.0.1:9126. This flag accepts a port, a host, or both (e.g.,
    /// `--graphql-metrics-address=9127`, `--graphql-metrics-address=0.0.0.0`,
    /// or `--graphql-metrics-address=0.0.0.0:9127`). It has no effect without
    /// `--with-graphql`.
    #[arg(long, value_name = "GRAPHQL_METRICS_HOST_PORT")]
    graphql_metrics_address: Option<String>,
    /// Port for the Indexer Postgres DB. Default port is 5432.
    #[arg(long, default_value = "5432")]
    pg_port: u16,
    /// Hostname for the Indexer Postgres DB. Default host is localhost.
    #[arg(long, default_value = "localhost")]
    pg_host: String,
    /// DB name for the Indexer Postgres DB. Default DB name is iota_indexer.
    #[arg(long, default_value = "iota_indexer")]
    pg_db_name: String,
    /// DB username for the Indexer Postgres DB. Default username is postgres.
    #[arg(long, default_value = "postgres")]
    pg_user: String,
    /// DB password for the Indexer Postgres DB. Default password is postgrespw.
    #[arg(long, default_value = "postgrespw")]
    pg_password: String,
    /// Retention options for the indexer writer. By default the indexer keeps
    /// all data, so its database grows without bound.
    /// Pass `--pruning-config-path <PATH>` to point at a TOML retention config
    /// (same format as the `iota-indexer indexer` command) to enable pruning.
    #[command(flatten)]
    pruning_options: PruningOptions,
}

#[cfg(feature = "indexer")]
impl IndexerFeatureArgs {
    /// Create a default instance for testing. Only used in integration tests.
    pub fn for_testing() -> Self {
        Self {
            with_indexer: None,
            with_graphql: None,
            graphql_metrics_address: None,
            pg_port: 5432,
            pg_host: "localhost".to_string(),
            pg_db_name: "iota_indexer".to_string(),
            pg_user: "postgres".to_string(),
            pg_password: "postgrespw".to_string(),
            pruning_options: PruningOptions::default(),
        }
    }
}

#[derive(Parser)]
pub enum LocalnetCommand {
    /// Start a local network in two modes: saving state between re-runs and not
    /// saving state between re-runs. Please use (--help) to see the full
    /// description.
    ///
    /// By default, iota-localnet start will start a local network from the
    /// genesis blob that exists in the IOTA config default dir or in the
    /// config_dir that was passed. If the default directory does not exist and
    /// the config_dir is not passed, it will generate a new default directory,
    /// generate the genesis blob, and start the network.
    ///
    /// Note that if you want to start an indexer, Postgres DB is required.
    ///
    /// Protocol config parameters can be overridden individually by setting
    /// environment variables as follows:
    /// - IOTA_PROTOCOL_CONFIG_OVERRIDE_ENABLE=1
    /// - Then, to configure an override, use the prefix
    ///   `IOTA_PROTOCOL_CONFIG_OVERRIDE_` along with the parameter name. For
    ///   example, to increase the interval between checkpoint creation to >1/s,
    ///   you might set:
    ///   IOTA_PROTOCOL_CONFIG_OVERRIDE_min_checkpoint_interval_ms=1000
    ///
    /// Note that protocol config parameters must match between all nodes, or
    /// the network may break. Changing these values outside of local
    /// networks is very dangerous.
    #[command(verbatim_doc_comment)]
    Start {
        /// Config directory that will be used to store network config, node db,
        /// keystore.
        /// `iota-localnet genesis -f --with-faucet` generates a genesis config
        /// that can be used to start this process. Use with caution as the `-f`
        /// flag will overwrite the existing config directory. We can use any
        /// config dir that is generated by the `iota-localnet genesis`.
        #[arg(long = "network.config")]
        config_dir: Option<std::path::PathBuf>,
        /// A new genesis is created each time this flag is set, and state is
        /// not persisted between runs. Only use this flag when you want
        /// to start the network from scratch every time you
        /// run this command.
        ///
        /// To run with persisted state, do not pass this flag and use the
        /// `iota-localnet genesis` command to generate a genesis that can be
        /// used to start the network with.
        #[arg(long)]
        force_regenesis: bool,
        /// Start a faucet with default host and port: 0.0.0.0:9123. This flag
        /// accepts also a port, a host, or both (e.g., 0.0.0.0:9123).
        /// When providing a specific value, please use the = sign between the
        /// flag and value: `--with-faucet=6124` or
        /// `--with-faucet=0.0.0.0`, or `--with-faucet=0.0.0.0:9123`
        #[arg(
            long,
            default_missing_value = "0.0.0.0:9123",
            num_args = 0..=1,
            require_equals = true,
            value_name = "FAUCET_HOST_PORT",
        )]
        with_faucet: Option<String>,
        /// Set the amount of nanos that the faucet will put in an object.
        /// Defaults to `200000000000`(200 IOTA).
        #[arg(long)]
        faucet_amount: Option<u64>,
        /// Set the amount of coin objects the faucet will send for each
        /// request. Defaults to 5.
        #[arg(long)]
        faucet_coin_count: Option<usize>,
        /// DEPRECATED: alias for `--node-config-override
        /// fullnode:enable-grpc-api=true` plus `--node-config-override
        /// fullnode:grpc-api-config={address: <HOST:PORT>}`.
        ///
        /// Start the gRPC API server with default host and port: 0.0.0.0:50051.
        /// This flag accepts also a port, a host, or both (e.g.,
        /// 0.0.0.0:50051). When providing a specific value, please use
        /// the = sign between the flag and value: `--with-grpc=50052`
        /// or `--with-grpc=0.0.0.0`, or `--with-grpc=0.0.0.0:50051`
        #[arg(
            long,
            default_missing_value = "0.0.0.0:50051",
            num_args = 0..=1,
            require_equals = true,
            value_name = "GRPC_HOST_PORT",
        )]
        with_grpc: Option<String>,
        #[cfg(feature = "indexer")]
        #[command(flatten)]
        indexer_feature_args: Box<IndexerFeatureArgs>,
        /// Port to start the Fullnode RPC server on. Default port is 9000.
        #[arg(long, default_value = "9000")]
        fullnode_rpc_port: u16,
        /// Set the epoch duration. Can only be used when `--force-regenesis`
        /// flag is passed or if there's no genesis config and one will
        /// be auto-generated. When this flag is not set but
        /// `--force-regenesis` is set, the epoch duration will be set to 60
        /// seconds.
        #[arg(long)]
        epoch_duration_ms: Option<u64>,
        /// Make the fullnode dump executed checkpoints as files to this
        /// directory. This is incompatible with --no-full-node.
        ///
        /// If --with-indexer is set, this defaults to a temporary directory.
        #[cfg(feature = "indexer")]
        #[arg(long, value_name = "DATA_INGESTION_DIR")]
        data_ingestion_dir: Option<PathBuf>,
        /// Start the network without a fullnode
        #[arg(long)]
        no_full_node: bool,
        /// Keep the fullnode's full history instead of pruning old object
        /// versions and checkpoints.
        #[arg(long, conflicts_with = "no_full_node")]
        disable_fullnode_pruning: bool,
        /// Override a value in the generated node configs for this run, e.g.
        /// `fullnode:authority-store-pruning-config.num-epochs-to-retain=5`.
        ///
        /// Scope is `all` (default), `fullnode`, `validator` (all
        /// validators), or `validator-<N>`. The path uses the field names
        /// from the node config YAML. The option is repeatable, and later
        /// overrides win.
        ///
        /// Values are YAML. Quote a value that would parse as structure
        /// (e.g. `'[::1]:9000'`). An empty value or `null` clears the field.
        /// A mapping merges with the section. A list replaces the section.
        ///
        /// A validator's `network-address`, `p2p-config.external-address`
        /// and `primary-address` are in the genesis committee metadata and
        /// cannot be overridden. Re-run genesis to change them.
        ///
        /// Warning: the network breaks if you override a per-node value
        /// (e.g. `db-path`) for every node, or if you clear
        /// `p2p-config.seed-peers`.
        // Taken as a string and parsed in `start`: clap reports a rejected
        // value by echoing the whole argument, which may carry a credential.
        #[arg(long, value_name = "[SCOPE:]PATH=VALUE")]
        node_config_override: Vec<String>,
        /// Set the number of validators in the network.
        /// If a genesis was already generated with a specific number of
        /// validators, this will not override it; the user should recreate the
        /// genesis with the desired number of validators.
        #[arg(long, help = "The number of validators in the network.")]
        committee_size: Option<usize>,
        /// Write the node config every node of this run would start with to
        /// the given directory, then exit without starting the network.
        ///
        /// The configs are the ones the run would use, overrides included,
        /// and each is runnable with `iota-node --config-path`. Nothing reads
        /// them back: editing one does not change what `start` runs.
        #[arg(long, value_name = "DIR")]
        write_config: Option<PathBuf>,
    },
    /// Bootstrap and initialize a new IOTA network
    Genesis {
        #[arg(long, help = "Start genesis with a given config file")]
        from_config: Option<PathBuf>,
        #[arg(
            long,
            help = "Build a genesis config, write it to the specified path, and exit"
        )]
        write_config: Option<PathBuf>,
        #[arg(long)]
        working_dir: Option<PathBuf>,
        #[arg(short, long, help = "Forces overwriting existing configuration")]
        force: bool,
        #[arg(long)]
        epoch_duration_ms: Option<u64>,
        #[arg(long, help = "Set the genesis chain start timestamp in milliseconds")]
        chain_start_timestamp_ms: Option<u64>,
        #[arg(
            long,
            value_name = "ADDR",
            num_args(1..),
            value_delimiter = ',',
            help = "A list of ip addresses to generate a genesis suitable for benchmarks"
        )]
        benchmark_ips: Option<Vec<String>>,
        #[arg(
            long,
            help = "Creates an extra faucet configuration for iota persisted runs."
        )]
        with_faucet: bool,
        /// Set number of validators in the network.
        #[arg(
            long,
            help = "The number of validators in the network.",
            default_value_t = DEFAULT_COMMITTEE_SIZE
        )]
        committee_size: usize,
        #[arg(
            long,
            help = "Number of additional gas accounts to create for benchmarks (use for dedicated clients)"
        )]
        num_additional_gas_accounts: Option<usize>,
        /// Set `admin-interface-address` config. This flag
        /// accepts also a port, a host, or both (e.g., 0.0.0.0:1337).
        /// When providing a specific value, please use the = sign between the
        /// flag and value: `--admin-interface-address=1337` or
        /// `--admin-interface-address=0.0.0.0`, or
        /// `--admin-interface-address=0.0.0.0:1337`
        #[arg(long, require_equals = true, value_name = "ADMIN_INTERFACE_HOST_PORT")]
        admin_interface_address: Option<String>,
    },
}

impl LocalnetCommand {
    pub async fn execute(self) -> Result<(), anyhow::Error> {
        match self {
            LocalnetCommand::Start {
                config_dir,
                force_regenesis,
                with_faucet,
                faucet_amount,
                faucet_coin_count,
                with_grpc,
                #[cfg(feature = "indexer")]
                indexer_feature_args,
                fullnode_rpc_port,
                #[cfg(feature = "indexer")]
                data_ingestion_dir,
                no_full_node,
                disable_fullnode_pruning,
                node_config_override,
                committee_size,
                epoch_duration_ms,
                write_config,
            } => {
                start(
                    config_dir.clone(),
                    with_faucet,
                    faucet_amount,
                    faucet_coin_count,
                    with_grpc,
                    #[cfg(feature = "indexer")]
                    *indexer_feature_args,
                    force_regenesis,
                    epoch_duration_ms,
                    fullnode_rpc_port,
                    #[cfg(feature = "indexer")]
                    data_ingestion_dir,
                    no_full_node,
                    disable_fullnode_pruning,
                    node_config_override,
                    committee_size,
                    write_config,
                )
                .await
            }
            LocalnetCommand::Genesis {
                working_dir,
                force,
                from_config,
                write_config,
                epoch_duration_ms,
                chain_start_timestamp_ms,
                benchmark_ips,
                with_faucet,
                committee_size,
                num_additional_gas_accounts,
                admin_interface_address,
            } => {
                genesis(
                    from_config,
                    write_config,
                    working_dir,
                    force,
                    epoch_duration_ms,
                    chain_start_timestamp_ms,
                    benchmark_ips,
                    with_faucet,
                    committee_size,
                    num_additional_gas_accounts,
                    admin_interface_address,
                )
                .await
            }
        }
    }
}

/// Starts a local network with the given configuration.
async fn start(
    config_dir: Option<PathBuf>,
    with_faucet: Option<String>,
    faucet_amount: Option<u64>,
    faucet_coin_count: Option<usize>,
    with_grpc: Option<String>,
    #[cfg(feature = "indexer")] indexer_feature_args: IndexerFeatureArgs,
    force_regenesis: bool,
    epoch_duration_ms: Option<u64>,
    fullnode_rpc_port: u16,
    #[cfg(feature = "indexer")] mut data_ingestion_dir: Option<PathBuf>,
    no_full_node: bool,
    disable_fullnode_pruning: bool,
    node_config_override: Vec<String>,
    committee_size: Option<usize>,
    write_config: Option<PathBuf>,
) -> Result<(), anyhow::Error> {
    // Parsed here rather than by clap, whose error would echo the whole
    // argument. These errors name the path only.
    let mut node_config_overrides = node_config_override
        .iter()
        .map(|input| input.parse::<NodeConfigOverride>())
        .collect::<Result<Vec<_>, _>>()?;

    if force_regenesis {
        ensure!(
            config_dir.is_none(),
            "Cannot pass `--force-regenesis` and `--network.config` at the same time."
        );
    }

    if with_grpc.is_some() {
        ensure!(!no_full_node, "Cannot enable gRPC without a fullnode.");
    }

    if write_config.is_some() {
        ensure!(
            with_faucet.is_none(),
            "Cannot pass `--with-faucet` and `--write-config` at the same time: the faucet is a \
             service, and `--write-config` starts nothing."
        );
        ensure!(
            !force_regenesis,
            "Cannot pass `--force-regenesis` and `--write-config` at the same time: the written \
             configs would point at a temporary directory that is removed on exit. Pass \
             `--network.config <DIR>` or run `iota-localnet genesis` first."
        );
        #[cfg(feature = "indexer")]
        ensure!(
            data_ingestion_dir.is_some()
                || (indexer_feature_args.with_indexer.is_none()
                    && indexer_feature_args.with_graphql.is_none()),
            "Cannot pass `--with-indexer` or `--with-graphql` and `--write-config` at the same \
             time: the written fullnode config would point at a temporary data ingestion \
             directory that is removed on exit. Pass `--data-ingestion-dir <DIR>`."
        );
    }

    #[cfg(feature = "indexer")]
    let IndexerFeatureArgs {
        mut with_indexer,
        with_graphql,
        graphql_metrics_address,
        pg_port,
        pg_host,
        pg_db_name,
        pg_user,
        pg_password,
        pruning_options,
    } = indexer_feature_args;

    #[cfg(feature = "indexer")]
    if with_graphql.is_some() {
        with_indexer = Some(with_indexer.unwrap_or_default());
    }

    #[cfg(feature = "indexer")]
    if with_indexer.is_some() {
        ensure!(
            !no_full_node,
            "Cannot start the indexer without a fullnode."
        );
    }

    if epoch_duration_ms.is_some() && genesis_blob_exists(config_dir.clone()) && !force_regenesis {
        bail!(
            "epoch duration can only be set when passing the `--force-regenesis` flag, or when \
            there is no genesis configuration in the default IOTA configuration folder or the given \
            network.config argument.",
        );
    }

    // Resolve the configuration directory.
    let config_path = config_dir.clone().map_or_else(iota_config_dir, Ok)?;

    // Deprecated --with-grpc becomes node config overrides. They are
    // prepended so that explicit --node-config-override values win.
    if let Some(input) = with_grpc.clone() {
        eprintln!(
            "{}",
            "[warning] The --with-grpc flag is deprecated. Use `--node-config-override \
             fullnode:enable-grpc-api=true`, which serves the API at the default address, \
             and add `--node-config-override fullnode:grpc-api-config={address: <HOST:PORT>}` \
             to serve it elsewhere."
                .yellow()
                .bold()
        );
        node_config_overrides.splice(0..0, with_grpc_overrides(input)?);
    }

    let mut swarm_builder = Swarm::builder();
    // The entry the fullnode's config is derived from. A persisted network
    // keeps it, so that the fullnode's key pairs, ports and db path are the
    // same on every run.
    let mut fullnode_config_info = None;

    if disable_fullnode_pruning {
        swarm_builder = swarm_builder.with_disable_fullnode_pruning();
    }

    // If this is set, then no data will be persisted between runs, and a new
    // genesis will be generated each run.
    if force_regenesis {
        let committee_size = NonZeroUsize::new(committee_size.unwrap_or(DEFAULT_COMMITTEE_SIZE))
            .ok_or_else(|| anyhow!("Committee size must be at least 1."))?;

        swarm_builder = swarm_builder
            .committee_size(committee_size)
            .with_deterministic_validator_ports(VALIDATOR_PORT_BASE);
        let genesis_config = GenesisConfig::custom_genesis(1, 100);
        swarm_builder = swarm_builder.with_genesis_config(genesis_config);
        let epoch_duration_ms = epoch_duration_ms.unwrap_or(DEFAULT_EPOCH_DURATION_MS);
        swarm_builder = swarm_builder.with_epoch_duration_ms(epoch_duration_ms);
    } else {
        let network_config_path = config_path.join(IOTA_NETWORK_CONFIG);
        // Auto genesis if no configuration exists in the configuration directory.
        if !network_config_path.exists() {
            if !config_path.exists() {
                fs::create_dir(&config_path).map_err(|err| {
                    anyhow!(err).context(format!(
                        "Cannot create network config dir {}",
                        config_path.display()
                    ))
                })?;
            }
            genesis(
                None,
                None,
                Some(config_path.clone()),
                false,
                epoch_duration_ms,
                None,
                None,
                false,
                committee_size.unwrap_or(DEFAULT_COMMITTEE_SIZE),
                None,
                None,
            )
            .await
            .map_err(|e| anyhow!("{e}: {}. \n\n\
            If you are trying to run a local network without persisting the data (so a new genesis that is \
            randomly generated and will not be saved once the network is shut down), use --force-regenesis flag. \n\
            If you are trying to persist the network data and start from a new genesis, use iota-localnet genesis --help \
            to see how to generate a new genesis.", config_path.display()))?;
        } else if committee_size.is_some() {
            eprintln!(
                "{}",
                "[warning] The committee-size arg will be ignored as a network configuration \
                        already exists. To change the committee size, you'll have to adjust the \
                        network configuration file or regenerate a genesis with the desired \
                        committee size. See `iota-localnet genesis --help` for more information."
                    .yellow()
                    .bold()
            );
        }

        let mut persisted_network_config = PersistedNetworkConfig::read(&config_path)?;
        fullnode_config_info = persisted_network_config
            .genesis_config
            .fullnode_config_info
            .take();
        // The genesis blob is read, never rebuilt: it is the network's
        // identity, and the node configs only point at it.
        let genesis = Genesis::new_from_file(config_path.join(IOTA_GENESIS_FILENAME));
        let network_config = persisted_network_config.into_network_config(&config_path, genesis)?;

        swarm_builder = swarm_builder
            .dir(config_path.clone())
            .with_network_config(network_config);
    }

    // A derived node config is a real node's, so it gets the default
    // denial-of-service protection the swarm builders leave unset. Set in both
    // modes, and before the overrides, so `policy-config=null` still clears it.
    let policy_config = PolicyConfig::default_dos_protection_policy();
    swarm_builder = swarm_builder
        .with_validator_policy_config(Some(policy_config.clone()))
        .with_fullnode_policy_config(Some(policy_config));

    // the indexer and GraphQL services communicate with the fullnode via gRPC, we
    // must enable it by default.
    #[cfg(feature = "indexer")]
    if with_indexer.is_some() || with_graphql.is_some() {
        // The gRPC API config is given rather than left out. The builder
        // would otherwise put the API on a free port, which differs on every
        // run. A `--node-config-override` still wins over it.
        swarm_builder = swarm_builder
            .with_fullnode_enable_grpc_api(true)
            .with_fullnode_grpc_api_config(GrpcApiConfig::default());
    }

    // the indexer requires to set the fullnode's data ingestion directory
    // note that this overrides the default configuration that is set when running
    // the genesis command, which sets data_ingestion_dir to None.
    //
    // The directory is owned here until the network launches, so that
    // `--write-config`, which starts nothing, leaves none behind.
    #[cfg(feature = "indexer")]
    let data_ingestion_tempdir = if with_indexer.is_some() && data_ingestion_dir.is_none() {
        let tempdir = tempdir()?;
        data_ingestion_dir = Some(tempdir.path().to_path_buf());
        Some(tempdir)
    } else {
        None
    };

    #[cfg(feature = "indexer")]
    if let Some(ref dir) = data_ingestion_dir {
        swarm_builder = swarm_builder.with_data_ingestion_dir(dir.clone());
    }

    check_fullnode_override_scopes(&node_config_overrides, !no_full_node)?;
    swarm_builder = swarm_builder.with_node_config_overrides(node_config_overrides);

    let mut fullnode_url = iota_config::node::default_json_rpc_address();
    fullnode_url.set_port(fullnode_rpc_port);

    if no_full_node {
        swarm_builder = swarm_builder.with_fullnode_count(0);
    } else {
        swarm_builder = swarm_builder
            .with_fullnode_count(1)
            .with_fullnode_rpc_addr(fullnode_url)
            .with_fullnode_genesis_config(
                fullnode_config_info.unwrap_or_else(|| fullnode_genesis_config(&mut OsRng, None)),
            );
    }

    let mut swarm = tokio::task::spawn_blocking(move || swarm_builder.try_build()).await??;
    log_applied_node_config_overrides(&swarm);

    if let Some(directory) = write_config {
        return write_node_configs(&swarm, &directory);
    }

    // The fullnode writes to the data ingestion directory for as long as it
    // runs, so it outlives this function from here on.
    #[cfg(feature = "indexer")]
    if let Some(tempdir) = data_ingestion_tempdir {
        let _ = tempdir.keep();
    }

    swarm.launch().await?;
    // Let nodes connect to one another
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    info!("Cluster started");

    // Taken off the built config rather than off `--fullnode-rpc-port`, since an
    // override can move the address.
    let fullnode_url = swarm
        .fullnodes()
        .next()
        .map(|node| node.config().json_rpc_address)
        .unwrap_or(fullnode_url);
    // the indexer requires a fullnode url with protocol specified
    let fullnode_url = format!("http://{fullnode_url}");
    info!("Fullnode URL: {}", fullnode_url);

    // Reported off the built config rather than off `--with-grpc`, since an
    // override can turn the API on too.
    if let Some(grpc_url) = swarm.fullnodes().next().and_then(|node| {
        let config = node.config();
        config.enable_grpc_api.then(|| {
            config
                .grpc_api_config
                .as_ref()
                .map(|grpc| grpc.address)
                .unwrap_or_else(|| GrpcApiConfig::default().address)
        })
    }) {
        info!("gRPC URL: http://{grpc_url}");
    }

    #[cfg(feature = "indexer")]
    let pg_address = format!("postgres://{pg_user}:{pg_password}@{pg_host}:{pg_port}/{pg_db_name}");

    #[cfg(feature = "indexer")]
    let fullnode_grpc_url = {
        let socket_addr = swarm
            .fullnodes()
            .next()
            .and_then(|node| {
                node.config()
                    .grpc_api_config
                    .as_ref()
                    .map(|grpc| grpc.address)
            })
            .unwrap_or_else(|| GrpcApiConfig::default().address);
        format!("http://{socket_addr}")
    };

    #[cfg(feature = "indexer")]
    if let Some(input) = with_indexer {
        let indexer_address = parse_host_port(input, DEFAULT_INDEXER_PORT)
            .map_err(|_| anyhow!("Invalid indexer host and port"))?;
        tracing::info!("Starting the indexer service at {indexer_address}");
        // Start in writer mode
        start_test_indexer(
            pg_address.clone(),
            // reset the existing db
            true,
            None,
            fullnode_grpc_url.clone(),
            IndexerTypeConfig::writer_mode(Some(pruning_options)),
            data_ingestion_dir.clone(),
        )
        .await;
        info!("Indexer in writer mode started");

        // Start in reader mode
        start_test_indexer(
            pg_address.clone(),
            false,
            None,
            fullnode_grpc_url.clone(),
            IndexerTypeConfig::reader_mode(indexer_address.to_string()),
            data_ingestion_dir.clone(),
        )
        .await;
        info!("Indexer in reader mode started");

        // Start in analytical worker mode
        start_test_indexer(
            pg_address.clone(),
            false,
            None,
            fullnode_grpc_url.clone(),
            IndexerTypeConfig::AnalyticalWorker,
            data_ingestion_dir,
        )
        .await;
        info!("Indexer in analytical worker mode started");
    }

    #[cfg(feature = "indexer")]
    if let Some(input) = with_graphql {
        let graphql_address = parse_host_port(input, DEFAULT_GRAPHQL_PORT)
            .map_err(|_| anyhow!("Invalid graphql host and port"))?;
        tracing::info!("Starting the GraphQL service at {graphql_address}");
        // The metrics address the service picks by default collides with the
        // fullnode metrics endpoint, which binds `FULLNODE_PORT_BASE` before
        // this runs.
        let graphql_metrics_address = parse_host_port_with_default_host(
            graphql_metrics_address.unwrap_or_default(),
            &Ipv4Addr::LOCALHOST.to_string(),
            DEFAULT_GRAPHQL_METRICS_PORT,
        )
        .map_err(|_| anyhow!("Invalid graphql metrics host and port"))?;
        // The service rebuilds the address as `host:port`, which an IPv6 host
        // needs brackets to survive.
        ensure!(
            graphql_metrics_address.is_ipv4(),
            "graphql metrics configuration requires an IPv4 address"
        );
        tracing::info!("Serving the GraphQL metrics at {graphql_metrics_address}");
        let graphql_connection_config = ConnectionConfig {
            port: graphql_address.port(),
            host: graphql_address.ip().to_string(),
            db_url: pg_address,
            prom_host: graphql_metrics_address.ip().to_string(),
            prom_port: graphql_metrics_address.port(),
            ..Default::default()
        };
        start_graphql_server_with_fn_rpc(
            graphql_connection_config,
            Some(fullnode_grpc_url),
            None, // it will be initialized by default
            None, // resolves to default service config
        )
        .await;
        info!("GraphQL started");
    }

    if let Some(input) = with_faucet {
        let faucet_address = parse_host_port(input, DEFAULT_FAUCET_PORT)
            .map_err(|_| anyhow!("Invalid faucet host and port"))?;
        tracing::info!("Starting the faucet service at {faucet_address}");
        let faucet_config_dir = if force_regenesis {
            // tempdir is used so the faucet file is cleaned up afterwards
            tempdir()?.keep()
        } else {
            config_path
        };

        let host_ip = match faucet_address {
            SocketAddr::V4(addr) => *addr.ip(),
            _ => bail!("faucet configuration requires an IPv4 address"),
        };

        let config = FaucetConfig {
            host_ip,
            port: faucet_address.port(),
            num_coins: faucet_coin_count.unwrap_or(DEFAULT_FAUCET_NUM_COINS),
            amount: faucet_amount.unwrap_or(DEFAULT_FAUCET_NANOS_AMOUNT),
            ..Default::default()
        };

        let prometheus_registry = prometheus_filtered::Registry::new();
        if force_regenesis {
            let kp = swarm.config_mut().account_keys.swap_remove(0);
            let keystore_path = faucet_config_dir.join(IOTA_KEYSTORE_FILENAME);
            let mut keystore = Keystore::from(FileBasedKeystore::new(&keystore_path).unwrap());
            let kp = SimpleKeypair::from(kp);
            let address: Address = kp.public_key().derive_address();
            keystore.add_key(None, kp).unwrap();
            IotaClientConfig::new(keystore)
                .with_envs([IotaEnv::new("localnet", fullnode_url)])
                .with_active_address(address)
                .with_active_env("localnet".to_string())
                .persisted(faucet_config_dir.join(IOTA_CLIENT_CONFIG).as_path())
                .save()
                .unwrap();
        }
        let faucet_wal = faucet_config_dir.join("faucet.wal");
        let simple_faucet = SimpleFaucet::new(
            create_wallet_context(config.wallet_client_timeout_secs, faucet_config_dir)?,
            &prometheus_registry,
            faucet_wal.as_path(),
            config.clone(),
        )
        .await
        .unwrap();

        let app_state = Arc::new(AppState {
            faucet: simple_faucet,
            config,
        });

        start_faucet(app_state, CONCURRENCY_LIMIT, &prometheus_registry).await?;
    }

    let mut interval = tokio::time::interval(std::time::Duration::from_secs(3));
    let mut unhealthy_cnt = 0;
    loop {
        for node in swarm.validator_nodes() {
            if let Err(err) = node.health_check(true).await {
                unhealthy_cnt += 1;
                if unhealthy_cnt > 3 {
                    // The network could temporarily go down during reconfiguration.
                    // If we detect a failed validator 3 times in a row, give up.
                    return Err(err.into());
                }
                // Break the inner loop so that we could retry latter.
                break;
            } else {
                unhealthy_cnt = 0;
            }
        }

        interval.tick().await;
    }
}

async fn genesis(
    from_config: Option<PathBuf>,
    write_config: Option<PathBuf>,
    working_dir: Option<PathBuf>,
    force: bool,
    epoch_duration_ms: Option<u64>,
    chain_start_timestamp_ms: Option<u64>,
    benchmark_ips: Option<Vec<String>>,
    with_faucet: bool,
    committee_size: usize,
    num_additional_gas_accounts: Option<usize>,
    admin_interface_address: Option<String>,
) -> Result<(), anyhow::Error> {
    let iota_config_dir = &match working_dir {
        // if a directory is specified, it must exist (it
        // will not be created)
        Some(v) => v,
        // create default IOTA config dir if not specified
        // on the command line and if it does not exist
        // yet
        None => iota_config_dir()?,
    };

    // if IOTA config dir is not empty then either clean it
    // up (if --force/-f option was specified or report an
    // error
    let dir = iota_config_dir.read_dir().map_err(|err| {
        anyhow!(err).context(format!("Cannot open IOTA config dir {iota_config_dir:?}"))
    })?;
    let files = dir.collect::<Result<Vec<_>, _>>()?;

    let client_path = iota_config_dir.join(IOTA_CLIENT_CONFIG);
    let keystore_path = iota_config_dir.join(IOTA_KEYSTORE_FILENAME);

    if write_config.is_none() && !files.is_empty() {
        if force {
            // check old keystore and client.yaml is compatible
            let is_compatible = FileBasedKeystore::new(&keystore_path).is_ok()
                && PersistedConfig::<IotaClientConfig>::read(&client_path).is_ok();
            // Keep keystore and client.yaml if they are compatible
            if is_compatible {
                for file in files {
                    let path = file.path();
                    if path != client_path && path != keystore_path {
                        if path.is_file() {
                            fs::remove_file(path)
                        } else {
                            fs::remove_dir_all(path)
                        }
                        .map_err(|err| {
                            anyhow!(err)
                                .context(format!("Cannot remove file {}", file.path().display()))
                        })?;
                    }
                }
            } else {
                fs::remove_dir_all(iota_config_dir).map_err(|err| {
                    anyhow!(err).context(format!(
                        "Cannot remove IOTA config dir {}",
                        iota_config_dir.display()
                    ))
                })?;
                fs::create_dir(iota_config_dir).map_err(|err| {
                    anyhow!(err).context(format!(
                        "Cannot create IOTA config dir {}",
                        iota_config_dir.display()
                    ))
                })?;
            }
        } else if files.len() != 2 || !client_path.exists() || !keystore_path.exists() {
            bail!(
                "Cannot run genesis with non-empty IOTA config directory {}. \n
                Please use the --force/-f option to remove the existing configuration",
                iota_config_dir.display()
            );
        }
    }

    let network_path = iota_config_dir.join(IOTA_NETWORK_CONFIG);
    let genesis_path = iota_config_dir.join(IOTA_GENESIS_FILENAME);

    let mut genesis_conf = match from_config {
        Some(path) => PersistedConfig::read(&path)?,
        None => {
            if let Some(ips) = benchmark_ips {
                // Make a keystore containing the key for the genesis gas object.
                let path = iota_config_dir.join(IOTA_BENCHMARK_GENESIS_GAS_KEYSTORE_FILENAME);
                let mut keystore = FileBasedKeystore::new(&path)?;
                let num_validators = ips.len();
                let num_accounts = num_validators + num_additional_gas_accounts.unwrap_or(0);
                for gas_key in GenesisConfig::benchmark_gas_keys(num_accounts) {
                    keystore.add_key(None, gas_key)?;
                }
                keystore.save()?;

                // Calculate extra allocations (validator, faucet)
                let validator_low_stake_threshold = GenesisConfig::default()
                    .protocol_config()
                    .validator_low_stake_threshold();
                let validator_extra = num_validators as u64
                    * (iota_swarm_config::genesis_config::DEFAULT_GAS_AMOUNT
                        + validator_low_stake_threshold);
                let mut faucet_extra = 0u64;
                if with_faucet {
                    faucet_extra = iota_swarm_config::genesis_config::DEFAULT_GAS_AMOUNT
                        * iota_swarm_config::genesis_config::DEFAULT_NUMBER_OF_OBJECT_PER_ACCOUNT
                            as u64;
                }
                // `u64::MAX - 1` is the max total supply value acceptable by
                // `iota::balance::increase_supply`
                let total_available_amount = (u64::MAX - 1)
                    .saturating_sub(validator_extra)
                    .saturating_sub(faucet_extra);

                // Make a new genesis config from the provided ip addresses with given epoch
                // duration and timestamp.
                GenesisConfig::new_for_benchmarks(
                    &ips,
                    epoch_duration_ms,
                    chain_start_timestamp_ms,
                    num_additional_gas_accounts,
                    total_available_amount,
                )
            } else if keystore_path.exists() {
                let existing_keys = FileBasedKeystore::new(&keystore_path)?.addresses();
                GenesisConfig::for_local_testing_with_addresses(existing_keys)
            } else {
                GenesisConfig::for_local_testing()
            }
        }
    };

    // Adds an extra faucet account to the genesis
    if with_faucet {
        info!("Adding faucet account in genesis config...");
        genesis_conf = genesis_conf.add_faucet_account();
    }

    if let Some(path) = write_config {
        let persisted = genesis_conf.persisted(&path);
        persisted.save()?;
        return Ok(());
    }

    let validator_info = genesis_conf.validator_config_info.take();
    let ssfn_info = genesis_conf.ssfn_config_info.take();
    // A genesis config that names its validators is a deployment's, and the
    // node config files of that deployment are written below. A plain `genesis`
    // names none and writes none: the configs a local network runs come from
    // `start --write-config`.
    let write_deployment_configs = validator_info.is_some();

    if let Some(epoch_duration_ms) = epoch_duration_ms {
        genesis_conf.parameters.epoch_duration_ms = epoch_duration_ms;
    }

    let admin_interface_address_with_port = admin_interface_address
        .map(|input| {
            let default_port = iota_config::node::default_admin_interface_address().port();
            parse_host_port(input, default_port)
                .map_err(|_| anyhow!("Invalid admin interface host and port"))
        })
        .transpose()?;

    let mut builder = ConfigBuilder::new(iota_config_dir)
        .with_genesis_config(genesis_conf)
        .with_empty_validator_genesis();
    builder = if let Some(validators) = validator_info {
        builder.with_validators(validators)
    } else {
        builder
            .committee_size(NonZeroUsize::new(committee_size).unwrap())
            .with_deterministic_ports(VALIDATOR_PORT_BASE)
    };

    if let Some(address) = admin_interface_address_with_port {
        builder = builder.with_admin_interface_address(address);
    }

    let (network_config, mut genesis_config) =
        tokio::task::spawn_blocking(move || builder.build_with_genesis_config()).await?;
    let mut keystore = FileBasedKeystore::new(&keystore_path)?;
    for key in &network_config.account_keys {
        keystore.add_key(None, SimpleKeypair::from(key.clone()))?;
    }
    let active_address = keystore.addresses().pop();

    let NetworkConfig {
        validator_configs,
        account_keys,
        genesis,
    } = network_config;
    genesis.save(&genesis_path)?;
    let genesis = Genesis::new_from_file(&genesis_path);

    // The fullnode is not part of the genesis committee, but its entry is
    // persisted with the validators'. `start` then derives the same fullnode
    // config, and reuses its database, on every run.
    genesis_config.fullnode_config_info.get_or_insert_with(|| {
        fullnode_genesis_config(&mut OsRng, admin_interface_address_with_port)
    });

    info!("Network genesis completed.");
    PersistedNetworkConfig {
        version: PersistedNetworkConfig::VERSION,
        genesis_config,
        account_keys,
    }
    .save(&network_path)?;
    info!("Network config file is stored in {:?}.", network_path);

    info!("Client keystore is stored in {:?}.", keystore_path);

    if write_deployment_configs {
        let ssfn_seed_peers = match ssfn_info {
            Some(ssfn_info) => write_state_sync_fullnode_configs(
                iota_config_dir,
                ssfn_info,
                &validator_configs,
                admin_interface_address_with_port,
            )?,
            None => Vec::new(),
        };
        write_validator_configs(
            iota_config_dir,
            validator_configs,
            &genesis,
            &ssfn_seed_peers,
        )?;
    }

    let mut client_config = if client_path.exists() {
        PersistedConfig::read(&client_path)?
    } else {
        IotaClientConfig::new(keystore).with_default_envs()
    };

    if client_config.active_address().is_none() {
        client_config.set_active_address(active_address);
    }

    // On windows, using 0.0.0.0 will usually yield in an networking error. This
    // localnet ip address must bind to 127.0.0.1 if the default 0.0.0.0 is
    // used.
    let json_rpc_address = iota_config::node::default_json_rpc_address();
    let localnet_ip = if json_rpc_address.ip() == IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)) {
        "127.0.0.1".to_string()
    } else {
        json_rpc_address.ip().to_string()
    };
    client_config.set_env(IotaEnv::new(
        "localnet",
        format!("http://{}:{}", localnet_ip, json_rpc_address.port()),
    ));
    client_config.add_env(IotaEnv::devnet());

    if client_config.active_env().is_none() {
        client_config.set_active_env(client_config.envs().first().map(|env| env.alias().clone()));
    }

    client_config.save(&client_path)?;
    info!("Client config file is stored in {:?}.", client_path);

    Ok(())
}

/// Build the genesis config entry the network's fullnode is derived from: its
/// key pairs, its metrics, admin interface and p2p addresses at the fixed
/// localnet ports, and a network address on a currently-free port.
///
/// Only the ports are fixed. The addresses stay on the IP the fullnode would
/// have used anyway. That IP is localhost outside the simulator, and one
/// address of its own inside it.
fn fullnode_genesis_config<R: rand::RngCore + rand::CryptoRng>(
    rng: &mut R,
    admin_interface_address: Option<SocketAddr>,
) -> ValidatorGenesisConfig {
    let mut config = ValidatorGenesisConfigBuilder::new().build(rng);
    let node_ip = config.network_address.to_socket_addr().unwrap().ip();
    config.metrics_address = SocketAddr::new(node_ip, FULLNODE_PORT_BASE);
    config.admin_interface_address =
        admin_interface_address.unwrap_or(SocketAddr::new(node_ip, FULLNODE_PORT_BASE + 1));
    config.p2p_address = local_ip_utils::new_deterministic_udp_address_for_testing(
        &node_ip.to_string(),
        FULLNODE_PORT_BASE + 2,
    );
    config
}

/// Write a node config file per state sync fullnode entry, and return the seed
/// peers the validators reach them through.
///
/// These are templates for a deployment, not configs a local network runs:
/// their paths and addresses are the ones a packaged node uses.
fn write_state_sync_fullnode_configs(
    config_directory: &Path,
    ssfn_info: Vec<SsfnGenesisConfig>,
    validator_configs: &[NodeConfig],
    admin_interface_address: Option<SocketAddr>,
) -> Result<Vec<SeedPeer>, anyhow::Error> {
    let mut ssfn_configs = vec![];
    for (index, ssfn) in ssfn_info.into_iter().enumerate() {
        let path = config_directory.join(iota_config::ssfn_config_file(
            ssfn.p2p_address.clone(),
            index,
        ));
        // join base fullnode config with each SsfnGenesisConfig entry
        let deployed_genesis = Genesis::new_from_file("/opt/iota/config/genesis.blob");
        let ssfn_config = FullnodeConfigBuilder::new()
            .with_config_directory(config_directory.to_path_buf())
            .with_p2p_external_address(ssfn.p2p_address)
            .with_network_key_pair(ssfn.network_key_pair)
            .with_p2p_listen_address(([0, 0, 0, 0], 8084))
            .with_db_path(PathBuf::from("/opt/iota/db/authorities_db/full_node_db"))
            .with_network_address("/ip4/0.0.0.0/tcp/8080/http".parse()?)
            .with_metrics_address(([0, 0, 0, 0], 9184))
            .with_admin_interface_address(admin_interface_address)
            .with_json_rpc_address(([0, 0, 0, 0], 9000))
            .with_genesis(deployed_genesis.clone())
            .with_policy_config(Some(PolicyConfig::default_dos_protection_policy()))
            .try_build_from_parts(&mut OsRng, validator_configs, deployed_genesis)?;
        ssfn_config.save(path)?;
        ssfn_configs.push(ssfn_config);
    }

    ssfn_configs
        .iter()
        .enumerate()
        .map(|(index, config)| {
            let address = config.p2p_config.external_address.clone().ok_or_else(|| {
                anyhow!(
                    "state sync fullnode {index} has no `p2p-config.external-address`, which the \
                     validators need to derive their seed peers"
                )
            })?;
            Ok(SeedPeer {
                peer_id: Some(anemo::PeerId(
                    config.network_key_pair().public().0.to_bytes(),
                )),
                address,
            })
        })
        .collect()
}

/// Write a node config file per validator, naming `ssfn_seed_peers` as their
/// seed peers.
///
/// These are templates for a deployment, not configs a local network runs:
/// their paths and addresses are the ones a packaged node uses.
fn write_validator_configs(
    config_directory: &Path,
    validator_configs: Vec<NodeConfig>,
    genesis: &Genesis,
    ssfn_seed_peers: &[SeedPeer],
) -> Result<(), anyhow::Error> {
    for (index, mut validator) in validator_configs.into_iter().enumerate() {
        let path = config_directory.join(iota_config::validator_config_file(
            validator.network_address.clone(),
            index,
        ));
        validator.genesis = genesis.clone();
        validator.policy_config = Some(PolicyConfig::default_dos_protection_policy());
        validator.p2p_config.seed_peers = ssfn_seed_peers.to_vec();
        validator.save(path)?;
    }
    Ok(())
}

/// Prepended to every node config `--write-config` writes. `serde_yaml` drops
/// comments, so the file is this text followed by the serialized config.
const NODE_CONFIG_HEADER: &str = "\
# Generated by `iota-localnet start --write-config`. Editing this file does not
# change what `iota-localnet start` runs; pass `--node-config-override` to that
# command instead.
#
# A key that is absent from this file is at its default. For `policy-config`
# and `grpc-api-config` an explicit `null`, not an absent key, is what turns
# the feature off.
#
# The key pairs below are not redacted: a node config only runs with them, and
# they are the keys a persisted network already holds in plaintext in its
# `network.yaml`.
";

/// Write the config of every node of `swarm` to `directory`, under the file
/// names the config directory of a local network uses.
fn write_node_configs(swarm: &Swarm, directory: &Path) -> Result<(), anyhow::Error> {
    fs::create_dir_all(directory).map_err(|err| {
        anyhow!(err).context(format!(
            "Cannot create node config dir {}",
            directory.display()
        ))
    })?;

    for (index, config) in swarm.config().validator_configs().iter().enumerate() {
        let path = directory.join(iota_config::validator_config_file(
            config.network_address.clone(),
            index,
        ));
        write_node_config(config, &path)?;
    }

    // The swarm keeps its nodes in a map, so sort them to give the same node
    // the same file name on every run.
    let mut fullnodes: Vec<&Node> = swarm.fullnodes().collect();
    fullnodes.sort_by_key(|fullnode| fullnode.name());
    for (index, fullnode) in fullnodes.iter().enumerate() {
        let name = if index == 0 {
            IOTA_FULLNODE_CONFIG.to_owned()
        } else {
            format!("fullnode-{index}.yaml")
        };
        write_node_config(&fullnode.config(), &directory.join(name))?;
    }

    info!("Node configs written to {}", directory.display());
    Ok(())
}

fn write_node_config(config: &NodeConfig, path: &Path) -> Result<(), anyhow::Error> {
    let mut contents = NODE_CONFIG_HEADER.to_owned();
    contents.push('\n');
    contents.push_str(&serde_yaml::to_string(config)?);
    fs::write(path, contents)
        .with_context(|| format!("Cannot write node config to {}", path.display()))
}

/// Translate the deprecated `--with-grpc` value into node config overrides.
///
/// The API is off on a generated fullnode, which leaves `grpc-api-config`
/// an explicit `null`. The address is therefore set with a value that
/// replaces the whole section. The fields that value does not mention come
/// out at their default.
fn with_grpc_overrides(input: String) -> Result<[NodeConfigOverride; 2], anyhow::Error> {
    let grpc_address = parse_host_port(input, DEFAULT_GRPC_PORT)
        .map_err(|_| anyhow!("Invalid gRPC host and port"))?;
    Ok([
        "fullnode:enable-grpc-api=true".parse()?,
        // Quoted: an IPv6 address like `[::1]:50051` is YAML structure
        // when unquoted.
        format!("fullnode:grpc-api-config={{address: '{grpc_address}'}}").parse()?,
    ])
}

/// Fails if a fullnode-scoped override is given for a network without a
/// fullnode.
fn check_fullnode_override_scopes(
    node_config_overrides: &[NodeConfigOverride],
    has_fullnode: bool,
) -> Result<(), anyhow::Error> {
    if !has_fullnode {
        for config_override in node_config_overrides {
            ensure!(
                config_override.scope != OverrideScope::Fullnode,
                "`{}` is fullnode-scoped, but this network has no fullnode",
                config_override.scoped_field_path()
            );
        }
    }
    Ok(())
}

/// Log, per node the overrides reached, how many applied and which fields
/// they set. Later overrides win per field.
fn log_applied_node_config_overrides(swarm: &Swarm) {
    let node_config_overrides = swarm.node_config_overrides();
    for index in 0..swarm.config().validator_configs().len() {
        log_applied_overrides_for_node(
            &format!("validator-{index}"),
            overrides_for_validator(node_config_overrides, index),
        );
    }
    if swarm.fullnodes().next().is_some() {
        log_applied_overrides_for_node("fullnode", overrides_for_fullnode(node_config_overrides));
    }
}

fn log_applied_overrides_for_node<'a>(
    name: &str,
    node_config_overrides: impl IntoIterator<Item = &'a NodeConfigOverride>,
) {
    let node_config_overrides: Vec<&NodeConfigOverride> =
        node_config_overrides.into_iter().collect();
    if node_config_overrides.is_empty() {
        return;
    }
    // The values are not echoed: they may carry a credential, and the
    // fields alone say what the run deviates on.
    let fields = winning_field_paths(node_config_overrides.iter().copied());
    let noun = if node_config_overrides.len() == 1 {
        "override"
    } else {
        "overrides"
    };
    info!(
        "applied {} {noun} to {name}: {}",
        node_config_overrides.len(),
        fields.join(", ")
    );
}

/// Parse the input string into a SocketAddr, with a default port if none is
/// provided.
pub fn parse_host_port(
    input: String,
    default_port_if_missing: u16,
) -> Result<SocketAddr, AddrParseError> {
    parse_host_port_with_default_host(input, "0.0.0.0", default_port_if_missing)
}

/// Same as [`parse_host_port`], for an endpoint whose host defaults to
/// something other than `0.0.0.0`.
pub fn parse_host_port_with_default_host(
    input: String,
    default_host: &str,
    default_port_if_missing: u16,
) -> Result<SocketAddr, AddrParseError> {
    let mut input = input;
    if input.contains("localhost") {
        input = input.replace("localhost", "127.0.0.1");
    }
    if input.contains(':') {
        input.parse::<SocketAddr>()
    } else if input.contains('.') {
        format!("{input}:{default_port_if_missing}").parse::<SocketAddr>()
    } else if !input.is_empty() {
        format!("{default_host}:{input}").parse::<SocketAddr>()
    } else {
        format!("{default_host}:{default_port_if_missing}").parse::<SocketAddr>()
    }
}

#[cfg(test)]
mod tests {
    use iota_swarm_config::node_config_override::apply_node_config_overrides;

    use super::*;

    #[test]
    fn fullnode_scoped_override_without_a_fullnode_fails() {
        let config_override: NodeConfigOverride =
            "fullnode:enable-index-processing=false".parse().unwrap();
        assert!(
            check_fullnode_override_scopes(std::slice::from_ref(&config_override), false).is_err()
        );
        check_fullnode_override_scopes(&[config_override], true).unwrap();

        // `all:` reaches the validators, so it stays allowed either way.
        let all_scoped: NodeConfigOverride = "all:enable-index-processing=false".parse().unwrap();
        check_fullnode_override_scopes(std::slice::from_ref(&all_scoped), false).unwrap();
    }

    #[test]
    fn with_grpc_overrides_turn_the_api_on_at_the_given_address() {
        let dir = tempdir().unwrap();
        for (input, expected) in [("[::1]:50051", "[::1]:50051"), ("50051", "0.0.0.0:50051")] {
            let overrides = with_grpc_overrides(input.to_string()).unwrap();
            let mut config = FullnodeConfigBuilder::new()
                .with_config_directory(dir.path().to_path_buf())
                .build_from_parts(&mut OsRng, &[], Genesis::new_empty());
            // One batch, as `start` hands them to the swarm builder.
            apply_node_config_overrides(&overrides, &mut config).unwrap();
            assert!(config.enable_grpc_api, "{input}");
            let grpc_api_config = config.grpc_api_config.unwrap();
            assert_eq!(
                grpc_api_config.address,
                expected.parse::<SocketAddr>().unwrap(),
                "{input}"
            );
            // Only the address deviates from the section's default.
            assert_eq!(
                grpc_api_config.max_message_size_bytes,
                GrpcApiConfig::default().max_message_size_bytes,
                "{input}"
            );
        }
    }

    /// Every service port of a local network is fixed, so a collision is a
    /// startup failure rather than a test flake. The fullnode metrics
    /// endpoint and the GraphQL metrics endpoint shared `9184` once.
    #[cfg(feature = "indexer")]
    #[test]
    fn service_ports_do_not_collide() {
        let mut ports = vec![
            DEFAULT_FAUCET_PORT,
            DEFAULT_GRPC_PORT,
            DEFAULT_GRAPHQL_PORT,
            DEFAULT_INDEXER_PORT,
            DEFAULT_GRAPHQL_METRICS_PORT,
            9000, // fullnode JSON-RPC, see `fullnode_rpc_port`
        ];
        // The fullnode owns `FULLNODE_PORT_BASE` and the two ports above it,
        // each validator the ten ports above its own base.
        ports.extend((0..3).map(|offset| FULLNODE_PORT_BASE + offset));
        ports.extend((0..10 * DEFAULT_COMMITTEE_SIZE as u16).map(|o| VALIDATOR_PORT_BASE + o));

        let unique = ports.iter().collect::<std::collections::BTreeSet<_>>();
        assert_eq!(unique.len(), ports.len(), "two services share a port");
    }

    /// An endpoint that keeps its host on localhost must stay there when only
    /// a port is given, and when nothing is given at all.
    #[cfg(feature = "indexer")]
    #[test]
    fn a_metrics_address_defaults_to_localhost() {
        let localhost = Ipv4Addr::LOCALHOST.to_string();
        let parse = |input: &str| {
            parse_host_port_with_default_host(
                input.to_string(),
                &localhost,
                DEFAULT_GRAPHQL_METRICS_PORT,
            )
            .unwrap()
        };

        assert_eq!(parse(""), SocketAddr::from(([127, 0, 0, 1], 9126)));
        assert_eq!(parse("9127"), SocketAddr::from(([127, 0, 0, 1], 9127)));
        assert_eq!(parse("localhost"), SocketAddr::from(([127, 0, 0, 1], 9126)));
        assert_eq!(parse("0.0.0.0"), SocketAddr::from(([0, 0, 0, 0], 9126)));
        assert_eq!(
            parse("0.0.0.0:9127"),
            SocketAddr::from(([0, 0, 0, 0], 9127))
        );
    }
}
