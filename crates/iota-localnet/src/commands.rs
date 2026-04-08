// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    fs,
    net::{AddrParseError, IpAddr, Ipv4Addr, SocketAddr},
    num::NonZeroUsize,
    path::PathBuf,
    sync::Arc,
};

use anyhow::{anyhow, bail, ensure};
use clap::*;
use colored::Colorize;
use fastcrypto::traits::KeyPair;
use iota_config::{
    Config, IOTA_BENCHMARK_GENESIS_GAS_KEYSTORE_FILENAME, IOTA_CLIENT_CONFIG, IOTA_FULLNODE_CONFIG,
    IOTA_GENESIS_FILENAME, IOTA_KEYSTORE_FILENAME, IOTA_NETWORK_CONFIG, NodeConfig,
    PersistedConfig, genesis_blob_exists, iota_config_dir,
    node::{Genesis, GrpcApiConfig},
    p2p::SeedPeer,
};
use iota_faucet::{AppState, FaucetConfig, SimpleFaucet, create_wallet_context, start_faucet};
use iota_genesis_builder::{SnapshotSource, SnapshotUrl};
#[cfg(feature = "indexer")]
use iota_graphql_rpc::{
    config::ConnectionConfig, test_infra::cluster::start_graphql_server_with_fn_rpc,
};
#[cfg(feature = "indexer")]
use iota_indexer::test_utils::{IndexerTypeConfig, start_test_indexer};
use iota_keys::keystore::{AccountKeystore, FileBasedKeystore, Keystore};
use iota_sdk::iota_client_config::{IotaClientConfig, IotaEnv};
use iota_swarm::memory::Swarm;
use iota_swarm_config::{
    genesis_config::GenesisConfig,
    network_config::{NetworkConfig, NetworkConfigLight},
    network_config_builder::ConfigBuilder,
    node_config_builder::FullnodeConfigBuilder,
};
use iota_types::{base_types::IotaAddress, crypto::IotaKeyPair};
use rand::rngs::OsRng;
use tempfile::tempdir;
use tracing::{info, warn};

const CONCURRENCY_LIMIT: usize = 30;
const DEFAULT_COMMITTEE_SIZE: usize = 1;
const DEFAULT_EPOCH_DURATION_MS: u64 = 60_000;
const DEFAULT_FAUCET_NUM_COINS: usize = 5;
const DEFAULT_FAUCET_NANOS_AMOUNT: u64 = 200_000_000_000; // 200 IOTA
const DEFAULT_FAUCET_PORT: u16 = 9123;
#[cfg(feature = "indexer")]
const DEFAULT_GRAPHQL_PORT: u16 = 9125;
#[cfg(feature = "indexer")]
const DEFAULT_INDEXER_PORT: u16 = 9124;

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
}

#[cfg(feature = "indexer")]
impl IndexerFeatureArgs {
    /// Create a default instance for testing. Only used in integration tests.
    pub fn for_testing() -> Self {
        Self {
            with_indexer: None,
            with_graphql: None,
            pg_port: 5432,
            pg_host: "localhost".to_string(),
            pg_db_name: "iota_indexer".to_string(),
            pg_user: "postgres".to_string(),
            pg_password: "postgrespw".to_string(),
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
        #[cfg(feature = "indexer")]
        #[command(flatten)]
        indexer_feature_args: IndexerFeatureArgs,
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
        /// Set the number of validators in the network.
        /// If a genesis was already generated with a specific number of
        /// validators, this will not override it; the user should recreate the
        /// genesis with the desired number of validators.
        #[arg(long, help = "The number of validators in the network.")]
        committee_size: Option<usize>,
        /// The path to local migration snapshot files
        #[arg(long, name = "path", num_args(0..))]
        local_migration_snapshots: Vec<PathBuf>,
        /// Remotely stored migration snapshots.
        #[arg(long, name = "iota|<full-url>", num_args(0..))]
        remote_migration_snapshots: Vec<SnapshotUrl>,
        #[arg(long, help = "Specify the delegator address")]
        delegator: Option<IotaAddress>,
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
        /// The path to local migration snapshot files
        #[arg(long, name = "path", num_args(0..))]
        local_migration_snapshots: Vec<PathBuf>,
        /// Remotely stored migration snapshots.
        #[arg(long, name = "iota|<full-url>", num_args(0..))]
        remote_migration_snapshots: Vec<SnapshotUrl>,
        #[arg(long, help = "Specify the delegator address")]
        delegator: Option<IotaAddress>,
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
                #[cfg(feature = "indexer")]
                indexer_feature_args,
                fullnode_rpc_port,
                #[cfg(feature = "indexer")]
                data_ingestion_dir,
                no_full_node,
                committee_size,
                epoch_duration_ms,
                local_migration_snapshots,
                remote_migration_snapshots,
                delegator,
            } => {
                start(
                    config_dir.clone(),
                    with_faucet,
                    faucet_amount,
                    faucet_coin_count,
                    #[cfg(feature = "indexer")]
                    indexer_feature_args,
                    force_regenesis,
                    epoch_duration_ms,
                    fullnode_rpc_port,
                    #[cfg(feature = "indexer")]
                    data_ingestion_dir,
                    no_full_node,
                    committee_size,
                    local_migration_snapshots,
                    remote_migration_snapshots,
                    delegator,
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
                local_migration_snapshots,
                remote_migration_snapshots,
                delegator,
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
                    local_migration_snapshots,
                    remote_migration_snapshots,
                    delegator,
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
    #[cfg(feature = "indexer")] indexer_feature_args: IndexerFeatureArgs,
    force_regenesis: bool,
    epoch_duration_ms: Option<u64>,
    fullnode_rpc_port: u16,
    #[cfg(feature = "indexer")] mut data_ingestion_dir: Option<PathBuf>,
    no_full_node: bool,
    committee_size: Option<usize>,
    local_migration_snapshots: Vec<PathBuf>,
    remote_migration_snapshots: Vec<SnapshotUrl>,
    delegator: Option<IotaAddress>,
) -> Result<(), anyhow::Error> {
    if force_regenesis {
        ensure!(
            config_dir.is_none(),
            "Cannot pass `--force-regenesis` and `--network.config` at the same time."
        );
    }

    #[cfg(feature = "indexer")]
    let IndexerFeatureArgs {
        mut with_indexer,
        with_graphql,
        pg_port,
        pg_host,
        pg_db_name,
        pg_user,
        pg_password,
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

    let mut swarm_builder = Swarm::builder();

    // If this is set, then no data will be persisted between runs, and a new
    // genesis will be generated each run.
    if force_regenesis {
        let committee_size = NonZeroUsize::new(committee_size.unwrap_or(DEFAULT_COMMITTEE_SIZE))
            .ok_or_else(|| anyhow!("Committee size must be at least 1."))?;

        swarm_builder = swarm_builder.committee_size(committee_size);
        let mut genesis_config = GenesisConfig::custom_genesis(1, 100);
        let local_snapshots = local_migration_snapshots
            .into_iter()
            .map(SnapshotSource::Local);
        let remote_snapshots = remote_migration_snapshots
            .into_iter()
            .map(SnapshotSource::S3);
        genesis_config.migration_sources = local_snapshots.chain(remote_snapshots).collect();

        // A delegator must be supplied when migration snapshots are provided.
        if !genesis_config.migration_sources.is_empty() {
            if let Some(delegator) = delegator {
                // Add a delegator account to the genesis.
                genesis_config = genesis_config.add_delegator(delegator);
            } else {
                bail!("a delegator must be supplied when migration snapshots are provided.");
            }
        }

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
                local_migration_snapshots,
                remote_migration_snapshots,
                delegator,
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

        let NetworkConfigLight {
            validator_configs,
            account_keys,
            ..
        } = PersistedConfig::read(&network_config_path).map_err(|err| {
            err.context(format!(
                "Cannot open IOTA network config file at {network_config_path:?}"
            ))
        })?;
        let first_validator_config = validator_configs.first().ok_or(anyhow!(
            "IOTA network config file must contain at least one validator config"
        ))?;
        let genesis = first_validator_config.genesis.clone();
        let migration_tx_data_path = first_validator_config.migration_tx_data_path.clone();
        ensure!(
            validator_configs
                .iter()
                .all(|config| genesis.eq(&config.genesis)),
            "All validators in IOTA network config must use the same genesis blob"
        );
        ensure!(
            validator_configs
                .iter()
                .all(|config| migration_tx_data_path.eq(&config.migration_tx_data_path)),
            "All validators in IOTA network config must use the same migration blob"
        );

        let fullnode_config_path = config_path.join(IOTA_FULLNODE_CONFIG);
        if fullnode_config_path.exists() {
            info!(
                "Loading IOTA-Names options from fullnode config file at {fullnode_config_path:?}"
            );

            let NodeConfig {
                iota_names_config,
                enable_grpc_api,
                grpc_api_config,
                db_path,
                genesis: fullnode_genesis,
                migration_tx_data_path: fullnode_migration_tx_data_path,
                ..
            } = PersistedConfig::read(&fullnode_config_path).map_err(|err| {
                err.context(format!(
                    "Cannot open fullnode config file at {fullnode_config_path:?}"
                ))
            })?;
            ensure!(
                genesis.eq(&fullnode_genesis),
                "Fullnode must use the same genesis blob as validators in IOTA network config"
            );
            ensure!(
                migration_tx_data_path.eq(&fullnode_migration_tx_data_path),
                "Fullnode must use the same migration blob as validators in IOTA network config"
            );
            swarm_builder = swarm_builder.with_fullnode_db_path(db_path);

            if let Some(iota_names_config) = iota_names_config {
                swarm_builder = swarm_builder.with_iota_names_config(iota_names_config);
            }

            swarm_builder = swarm_builder.with_fullnode_enable_grpc_api(enable_grpc_api);
            if enable_grpc_api {
                // Apply gRPC configuration if enabled
                if let Some(grpc_config) = grpc_api_config {
                    info!("Enabling gRPC API for fullnode with config: {grpc_config:?}");
                    swarm_builder = swarm_builder.with_fullnode_grpc_api_config(grpc_config);
                } else {
                    warn!("gRPC API enabled but no grpc-api-config provided, using default");
                    swarm_builder =
                        swarm_builder.with_fullnode_grpc_api_config(GrpcApiConfig::default());
                }
            }
        }

        let network_config = NetworkConfig {
            validator_configs,
            account_keys,
            genesis: genesis.genesis()?.clone(),
        };

        swarm_builder = swarm_builder
            .dir(config_path.clone())
            .with_network_config(network_config);
    }

    // the indexer and GraphQL services communicate with the fullnode via gRPC, we
    // must enable it by default.
    #[cfg(feature = "indexer")]
    if with_indexer.is_some() || with_graphql.is_some() {
        // the gRPC api uses default values if config is not provided,
        // allowing to not override it when provided in fullnode config.
        swarm_builder = swarm_builder.with_fullnode_enable_grpc_api(true);
    }

    // the indexer requires to set the fullnode's data ingestion directory
    // note that this overrides the default configuration that is set when running
    // the genesis command, which sets data_ingestion_dir to None.
    #[cfg(feature = "indexer")]
    if with_indexer.is_some() && data_ingestion_dir.is_none() {
        data_ingestion_dir = Some(tempdir()?.keep())
    }

    #[cfg(feature = "indexer")]
    if let Some(ref dir) = data_ingestion_dir {
        swarm_builder = swarm_builder.with_data_ingestion_dir(dir.clone());
    }

    let mut fullnode_url = iota_config::node::default_json_rpc_address();
    fullnode_url.set_port(fullnode_rpc_port);

    if no_full_node {
        swarm_builder = swarm_builder.with_fullnode_count(0);
    } else {
        swarm_builder = swarm_builder
            .with_fullnode_count(1)
            .with_fullnode_rpc_addr(fullnode_url);
    }

    let mut swarm = tokio::task::spawn_blocking(move || swarm_builder.build()).await?;
    swarm.launch().await?;
    // Let nodes connect to one another
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    info!("Cluster started");

    // the indexer requires a fullnode url with protocol specified
    let fullnode_url = format!("http://{fullnode_url}");
    info!("Fullnode URL: {}", fullnode_url);
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
            IndexerTypeConfig::writer_mode(None, None),
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
        let graphql_connection_config = ConnectionConfig {
            port: graphql_address.port(),
            host: graphql_address.ip().to_string(),
            db_url: pg_address,
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

        let prometheus_registry = prometheus::Registry::new();
        if force_regenesis {
            let kp = swarm.config_mut().account_keys.swap_remove(0);
            let keystore_path = faucet_config_dir.join(IOTA_KEYSTORE_FILENAME);
            let mut keystore = Keystore::from(FileBasedKeystore::new(&keystore_path).unwrap());
            let address: IotaAddress = kp.public().into();
            keystore.add_key(None, IotaKeyPair::Ed25519(kp)).unwrap();
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
    local_migration_snapshots: Vec<PathBuf>,
    remote_migration_snapshots: Vec<SnapshotUrl>,
    delegator: Option<IotaAddress>,
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
                let validator_extra = num_validators as u64
                    * (iota_swarm_config::genesis_config::DEFAULT_GAS_AMOUNT
                        + iota_types::governance::VALIDATOR_LOW_STAKE_THRESHOLD_NANOS);
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
    let local_snapshots = local_migration_snapshots
        .into_iter()
        .map(SnapshotSource::Local);
    let remote_snapshots = remote_migration_snapshots
        .into_iter()
        .map(SnapshotSource::S3);
    genesis_conf.migration_sources = local_snapshots.chain(remote_snapshots).collect();

    // A delegator must be supplied when migration snapshots are provided.
    if !genesis_conf.migration_sources.is_empty() {
        if let Some(delegator) = delegator {
            // Add a delegator account to the genesis.
            genesis_conf = genesis_conf.add_delegator(delegator);
        } else {
            bail!("a delegator must be supplied when migration snapshots are provided.");
        }
    }

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
        builder.committee_size(NonZeroUsize::new(committee_size).unwrap())
    };

    if let Some(address) = admin_interface_address_with_port {
        builder = builder.with_admin_interface_address(address);
    }

    let network_config = tokio::task::spawn_blocking(move || builder.build()).await?;
    let mut keystore = FileBasedKeystore::new(&keystore_path)?;
    for key in &network_config.account_keys {
        keystore.add_key(None, IotaKeyPair::Ed25519(key.copy()))?;
    }
    let active_address = keystore.addresses().pop();

    let NetworkConfig {
        validator_configs,
        account_keys,
        genesis,
    } = network_config;
    let mut network_config = NetworkConfigLight::new(validator_configs, account_keys, &genesis);
    genesis.save(&genesis_path)?;
    let genesis = iota_config::node::Genesis::new_from_file(&genesis_path);
    for validator in &mut network_config.validator_configs {
        validator.genesis = genesis.clone();
    }

    info!("Network genesis completed.");
    network_config.save(&network_path)?;
    info!("Network config file is stored in {:?}.", network_path);

    info!("Client keystore is stored in {:?}.", keystore_path);

    let fullnode_config = FullnodeConfigBuilder::new()
        .with_config_directory(iota_config_dir.to_path_buf())
        .with_rpc_addr(iota_config::node::default_json_rpc_address())
        .with_genesis(genesis.clone())
        .with_admin_interface_address(admin_interface_address_with_port)
        .build_from_parts(&mut OsRng, network_config.validator_configs(), genesis);

    fullnode_config.save(iota_config_dir.join(IOTA_FULLNODE_CONFIG))?;
    let mut ssfn_nodes = vec![];
    if let Some(ssfn_info) = ssfn_info {
        for (i, ssfn) in ssfn_info.into_iter().enumerate() {
            let path =
                iota_config_dir.join(iota_config::ssfn_config_file(ssfn.p2p_address.clone(), i));
            // join base fullnode config with each SsfnGenesisConfig entry
            let genesis = Genesis::new_from_file("/opt/iota/config/genesis.blob");
            let ssfn_config = FullnodeConfigBuilder::new()
                .with_config_directory(iota_config_dir.to_path_buf())
                .with_p2p_external_address(ssfn.p2p_address)
                .with_network_key_pair(ssfn.network_key_pair)
                .with_p2p_listen_address(([0, 0, 0, 0], 8084))
                .with_db_path(PathBuf::from("/opt/iota/db/authorities_db/full_node_db"))
                .with_network_address("/ip4/0.0.0.0/tcp/8080/http".parse()?)
                .with_metrics_address(([0, 0, 0, 0], 9184))
                .with_admin_interface_address(admin_interface_address_with_port)
                .with_json_rpc_address(([0, 0, 0, 0], 9000))
                .with_genesis(genesis.clone())
                .build_from_parts(&mut OsRng, network_config.validator_configs(), genesis);
            ssfn_nodes.push(ssfn_config.clone());
            ssfn_config.save(path)?;
        }

        let ssfn_seed_peers: Vec<SeedPeer> = ssfn_nodes
            .iter()
            .map(|config| SeedPeer {
                peer_id: Some(anemo::PeerId(
                    config.network_key_pair().public().0.to_bytes(),
                )),
                address: config.p2p_config.external_address.clone().unwrap(),
            })
            .collect();

        for (i, mut validator) in network_config
            .into_validator_configs()
            .into_iter()
            .enumerate()
        {
            let path = iota_config_dir.join(iota_config::validator_config_file(
                validator.network_address.clone(),
                i,
            ));
            let mut val_p2p = validator.p2p_config.clone();
            val_p2p.seed_peers.clone_from(&ssfn_seed_peers);
            validator.p2p_config = val_p2p;
            validator.save(path)?;
        }
    } else {
        for (i, validator) in network_config
            .into_validator_configs()
            .into_iter()
            .enumerate()
        {
            let path = iota_config_dir.join(iota_config::validator_config_file(
                validator.network_address.clone(),
                i,
            ));
            validator.save(path)?;
        }
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
    let localnet_ip =
        if fullnode_config.json_rpc_address.ip() == IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)) {
            "127.0.0.1".to_string()
        } else {
            fullnode_config.json_rpc_address.ip().to_string()
        };
    client_config.set_env(IotaEnv::new(
        "localnet",
        format!(
            "http://{}:{}",
            localnet_ip,
            fullnode_config.json_rpc_address.port()
        ),
    ));
    client_config.add_env(IotaEnv::devnet());

    if client_config.active_env().is_none() {
        client_config.set_active_env(client_config.envs().first().map(|env| env.alias().clone()));
    }

    client_config.save(&client_path)?;
    info!("Client config file is stored in {:?}.", client_path);

    Ok(())
}

/// Parse the input string into a SocketAddr, with a default port if none is
/// provided.
pub fn parse_host_port(
    input: String,
    default_port_if_missing: u16,
) -> Result<SocketAddr, AddrParseError> {
    let default_host = "0.0.0.0";
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
