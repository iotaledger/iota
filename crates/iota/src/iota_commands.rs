// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    fs, io,
    io::{Write, stderr, stdout},
    net::{AddrParseError, IpAddr, Ipv4Addr, SocketAddr},
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{anyhow, bail, ensure};
use clap::*;
use fastcrypto::traits::KeyPair;
use iota_bridge::{
    config::BridgeCommitteeConfig, iota_client::IotaBridgeClient,
    iota_transaction_builder::build_committee_register_transaction,
};
use iota_config::{
    Config, FULL_NODE_DB_PATH, IOTA_BENCHMARK_GENESIS_GAS_KEYSTORE_FILENAME, IOTA_CLIENT_CONFIG,
    IOTA_FULLNODE_CONFIG, IOTA_GENESIS_FILENAME, IOTA_KEYSTORE_FILENAME, IOTA_NETWORK_CONFIG,
    PersistedConfig, genesis_blob_exists, iota_config_dir, node::Genesis, p2p::SeedPeer,
};
use iota_faucet::{AppState, FaucetConfig, SimpleFaucet, create_wallet_context, start_faucet};
use iota_genesis_builder::{SnapshotSource, SnapshotUrl};
#[cfg(feature = "indexer")]
use iota_graphql_rpc::{
    config::ConnectionConfig, test_infra::cluster::start_graphql_server_with_fn_rpc,
};
#[cfg(feature = "indexer")]
use iota_indexer::test_utils::{ReaderWriterConfig, start_test_indexer};
use iota_keys::{
    keypair_file::read_key,
    keystore::{AccountKeystore, FileBasedKeystore, Keystore},
};
use iota_move::{self, execute_move_command};
use iota_move_build::IotaPackageHooks;
use iota_sdk::{
    iota_client_config::{IotaClientConfig, IotaEnv},
    wallet_context::WalletContext,
};
use iota_swarm::memory::Swarm;
use iota_swarm_config::{
    genesis_config::{DEFAULT_NUMBER_OF_AUTHORITIES, GenesisConfig},
    network_config::{NetworkConfig, NetworkConfigLight},
    network_config_builder::ConfigBuilder,
    node_config_builder::FullnodeConfigBuilder,
};
use iota_types::{
    base_types::IotaAddress,
    crypto::{IotaKeyPair, SignatureScheme, ToFromBytes},
};
use move_analyzer::analyzer;
use move_package::BuildConfig;
use rand::rngs::OsRng;
use tempfile::tempdir;
use tracing::{self, info};

use crate::{
    client_commands::IotaClientCommands,
    console::start_console,
    fire_drill::{FireDrill, run_fire_drill},
    genesis_ceremony::{Ceremony, run},
    keytool::KeyToolCommand,
    validator_commands::IotaValidatorCommand,
};

const CONCURRENCY_LIMIT: usize = 30;
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
#[command(rename_all = "kebab-case")]
pub enum IotaCommand {
    /// Start a local network in two modes: saving state between re-runs and not
    /// saving state between re-runs. Please use (--help) to see the full
    /// description.
    ///
    /// By default, iota start will start a local network from the genesis blob
    /// that exists in the IOTA config default dir or in the config_dir that
    /// was passed. If the default directory does not exist and the
    /// config_dir is not passed, it will generate a new default directory,
    /// generate the genesis blob, and start the network.
    ///
    /// Note that if you want to start an indexer, Postgres DB is required.
    #[command(name = "start")]
    Start {
        /// Config directory that will be used to store network config, node db,
        /// keystore.
        /// `iota genesis -f --with-faucet` generates a genesis config that can
        /// be used to start this process. Use with caution as the `-f` flag
        /// will overwrite the existing config directory. We can use any config
        /// dir that is generated by the `iota genesis`.
        #[arg(long = "network.config")]
        config_dir: Option<std::path::PathBuf>,

        /// A new genesis is created each time this flag is set, and state is
        /// not persisted between runs. Only use this flag when you want
        /// to start the network from scratch every time you
        /// run this command.
        ///
        /// To run with persisted state, do not pass this flag and use the `iota
        /// genesis` command to generate a genesis that can be used to
        /// start the network with.
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
        #[arg(long = "no-full-node")]
        no_full_node: bool,

        /// The path to local migration snapshot files
        #[arg(long, name = "path", num_args(0..))]
        local_migration_snapshots: Vec<PathBuf>,
        /// Remotely stored migration snapshots.
        #[arg(long, name = "iota|<full-url>", num_args(0..))]
        remote_migration_snapshots: Vec<SnapshotUrl>,
        #[arg(long, help = "Specify the delegator address")]
        delegator: Option<IotaAddress>,
    },
    /// Bootstrap and initialize a new iota network
    #[command(name = "genesis")]
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
        #[arg(long = "epoch-duration-ms")]
        epoch_duration_ms: Option<u64>,
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
        #[arg(
            long,
            help = "The number of validators in the network.",
            default_value_t = DEFAULT_NUMBER_OF_AUTHORITIES
        )]
        num_validators: usize,
        /// The path to local migration snapshot files
        #[arg(long, name = "path", num_args(0..))]
        local_migration_snapshots: Vec<PathBuf>,
        /// Remotely stored migration snapshots.
        #[arg(long, name = "iota|<full-url>", num_args(0..))]
        remote_migration_snapshots: Vec<SnapshotUrl>,
        #[arg(long, help = "Specify the delegator address")]
        delegator: Option<IotaAddress>,
    },
    /// Create an IOTA Genesis Ceremony with multiple remote validators.
    GenesisCeremony(Ceremony),
    /// IOTA keystore tool.
    #[command(name = "keytool")]
    KeyTool {
        #[arg(long)]
        keystore_path: Option<PathBuf>,
        /// Return command outputs in json format
        #[arg(long, global = true)]
        json: bool,
        /// Subcommands.
        #[command(subcommand)]
        cmd: KeyToolCommand,
    },
    /// Start IOTA interactive console.
    #[command(name = "console")]
    Console {
        /// Sets the file storing the state of our user accounts (an empty one
        /// will be created if missing)
        #[arg(long = "client.config")]
        config: Option<PathBuf>,
    },
    /// Client for interacting with the IOTA network.
    #[command(name = "client")]
    Client {
        /// Sets the file storing the state of our user accounts (an empty one
        /// will be created if missing)
        #[arg(long = "client.config")]
        config: Option<PathBuf>,
        #[command(subcommand)]
        cmd: Option<IotaClientCommands>,
        /// Return command outputs in json format.
        #[arg(long, global = true)]
        json: bool,
        #[arg(short = 'y', long = "yes")]
        accept_defaults: bool,
    },
    /// A tool for validators and validator candidates.
    #[command(name = "validator")]
    Validator {
        /// Sets the file storing the state of our user accounts (an empty one
        /// will be created if missing)
        #[arg(long = "client.config")]
        config: Option<PathBuf>,
        #[command(subcommand)]
        cmd: Option<IotaValidatorCommand>,
        /// Return command outputs in json format.
        #[arg(long, global = true)]
        json: bool,
        #[arg(short = 'y', long = "yes")]
        accept_defaults: bool,
    },
    /// Tool to build and test Move applications.
    #[command(name = "move")]
    Move {
        /// Path to a package which the command should be run with respect to.
        #[arg(long = "path", short = 'p', global = true)]
        package_path: Option<PathBuf>,
        /// Sets the file storing the state of our user accounts (an empty one
        /// will be created if missing) Only used when the
        /// `--dump-bytecode-as-base64` is set.
        #[arg(long = "client.config")]
        config: Option<PathBuf>,
        /// Package build options
        #[command(flatten)]
        build_config: BuildConfig,
        /// Subcommands.
        #[command(subcommand)]
        cmd: iota_move::Command,
    },
    /// Command to initialize the bridge committee, usually used when
    /// running local bridge cluster.
    #[command(name = "bridge-committee-init")]
    BridgeInitialize {
        #[arg(long = "network.config")]
        network_config: Option<PathBuf>,
        #[arg(long = "client.config")]
        client_config: Option<PathBuf>,
        #[arg(long = "bridge_committee.config")]
        bridge_committee_config_path: PathBuf,
    },
    /// Tool for Fire Drill
    FireDrill {
        #[command(subcommand)]
        fire_drill: FireDrill,
    },
    /// Invoke IOTA's move-analyzer via CLI
    #[command(name = "analyzer", hide = true)]
    Analyzer,
    /// Generate completion files for various shells
    #[cfg(feature = "gen-completions")]
    GenerateCompletions(crate::completions::GenerateCompletionsCommand),
}

impl IotaCommand {
    pub async fn execute(self) -> Result<(), anyhow::Error> {
        move_package::package_hooks::register_package_hooks(Box::new(IotaPackageHooks));
        match self {
            IotaCommand::Start {
                config_dir,
                force_regenesis,
                with_faucet,
                faucet_amount,
                #[cfg(feature = "indexer")]
                indexer_feature_args,
                fullnode_rpc_port,
                #[cfg(feature = "indexer")]
                data_ingestion_dir,
                no_full_node,
                epoch_duration_ms,
                local_migration_snapshots,
                remote_migration_snapshots,
                delegator,
            } => {
                start(
                    config_dir.clone(),
                    with_faucet,
                    faucet_amount,
                    #[cfg(feature = "indexer")]
                    indexer_feature_args,
                    force_regenesis,
                    epoch_duration_ms,
                    fullnode_rpc_port,
                    #[cfg(feature = "indexer")]
                    data_ingestion_dir,
                    no_full_node,
                    local_migration_snapshots,
                    remote_migration_snapshots,
                    delegator,
                )
                .await?;

                Ok(())
            }
            IotaCommand::Genesis {
                working_dir,
                force,
                from_config,
                write_config,
                epoch_duration_ms,
                benchmark_ips,
                with_faucet,
                num_validators,
                local_migration_snapshots: with_local_migration_snapshot,
                remote_migration_snapshots: with_remote_migration_snapshot,
                delegator,
            } => {
                genesis(
                    from_config,
                    write_config,
                    working_dir,
                    force,
                    epoch_duration_ms,
                    benchmark_ips,
                    with_faucet,
                    num_validators,
                    with_local_migration_snapshot,
                    with_remote_migration_snapshot,
                    delegator,
                )
                .await
            }
            IotaCommand::GenesisCeremony(cmd) => run(cmd).await,
            IotaCommand::KeyTool {
                keystore_path,
                json,
                cmd,
            } => {
                let keystore_path =
                    keystore_path.unwrap_or(iota_config_dir()?.join(IOTA_KEYSTORE_FILENAME));
                let mut keystore = Keystore::from(FileBasedKeystore::new(&keystore_path)?);
                cmd.execute(&mut keystore).await?.print(!json);
                Ok(())
            }
            IotaCommand::Console { config } => {
                let config = config.unwrap_or(iota_config_dir()?.join(IOTA_CLIENT_CONFIG));
                prompt_if_no_config(&config, false, true).await?;
                let context = WalletContext::new(&config, None, None)?;
                start_console(context, &mut stdout(), &mut stderr()).await
            }
            IotaCommand::Client {
                config,
                cmd,
                json,
                accept_defaults,
            } => {
                let config_path = config.unwrap_or(iota_config_dir()?.join(IOTA_CLIENT_CONFIG));
                prompt_if_no_config(
                    &config_path,
                    accept_defaults,
                    !matches!(cmd, Some(IotaClientCommands::NewAddress { .. })),
                )
                .await?;
                if let Some(cmd) = cmd {
                    let mut context = WalletContext::new(&config_path, None, None)?;
                    cmd.execute(&mut context).await?.print(!json);
                } else {
                    // Print help
                    let mut app: Command = IotaCommand::command();
                    app.build();
                    app.find_subcommand_mut("client").unwrap().print_help()?;
                }
                Ok(())
            }
            IotaCommand::Validator {
                config,
                cmd,
                json,
                accept_defaults,
            } => {
                let config_path = config.unwrap_or(iota_config_dir()?.join(IOTA_CLIENT_CONFIG));
                prompt_if_no_config(&config_path, accept_defaults, true).await?;
                let mut context = WalletContext::new(&config_path, None, None)?;
                if let Some(cmd) = cmd {
                    cmd.execute(&mut context).await?.print(!json);
                } else {
                    // Print help
                    let mut app: Command = IotaCommand::command();
                    app.build();
                    app.find_subcommand_mut("validator").unwrap().print_help()?;
                }
                Ok(())
            }
            IotaCommand::Move {
                package_path,
                build_config,
                mut cmd,
                config: client_config,
            } => {
                match &mut cmd {
                    iota_move::Command::Build(build) if build.dump_bytecode_as_base64 => {
                        if build.ignore_chain {
                            build.chain_id = None;
                        } else {
                            // `iota move build` does not ordinarily require a network connection.
                            // The exception is when --dump-bytecode-as-base64 is specified: In this
                            // case, we should resolve the correct addresses for the respective
                            // chain (e.g., testnet, mainnet) from the Move.lock under automated
                            // address management.
                            let config = client_config
                                .unwrap_or(iota_config_dir()?.join(IOTA_CLIENT_CONFIG));
                            prompt_if_no_config(&config, false, true).await?;
                            let context = WalletContext::new(&config, None, None)?;
                            let client = context.get_client().await?;
                            build.chain_id = client.read_api().get_chain_identifier().await.ok();
                        }
                    }
                    _ => (),
                };
                execute_move_command(package_path.as_deref(), build_config, cmd)
            }
            IotaCommand::BridgeInitialize {
                network_config,
                client_config,
                bridge_committee_config_path,
            } => {
                // Load the config of the IOTA authority.
                let network_config_path = network_config
                    .clone()
                    .unwrap_or(iota_config_dir()?.join(IOTA_NETWORK_CONFIG));
                let network_config: NetworkConfig = PersistedConfig::read(&network_config_path)
                    .map_err(|err| {
                        err.context(format!(
                            "Cannot open IOTA network config file at {:?}",
                            network_config_path
                        ))
                    })?;
                let bridge_committee_config: BridgeCommitteeConfig =
                    PersistedConfig::read(&bridge_committee_config_path).map_err(|err| {
                        err.context(format!(
                            "Cannot open Bridge Committee config file at {:?}",
                            bridge_committee_config_path
                        ))
                    })?;

                let config_path =
                    client_config.unwrap_or(iota_config_dir()?.join(IOTA_CLIENT_CONFIG));
                let mut context = WalletContext::new(&config_path, None, None)?;
                let rgp = context.get_reference_gas_price().await?;
                let rpc_url = context.config().get_active_env()?.rpc();
                println!("rpc_url: {}", rpc_url);
                let iota_bridge_client = IotaBridgeClient::new(rpc_url).await?;
                let bridge_arg = iota_bridge_client
                    .get_mutable_bridge_object_arg_must_succeed()
                    .await;
                assert_eq!(
                    network_config.validator_configs().len(),
                    bridge_committee_config
                        .bridge_authority_port_and_key_path
                        .len()
                );
                for node_config in network_config.validator_configs() {
                    let account_kp = node_config.account_key_pair.keypair();
                    context.add_account(None, account_kp.copy());
                }

                let context = context;
                let mut tasks = vec![];
                for (node_config, (port, key_path)) in network_config
                    .validator_configs()
                    .iter()
                    .zip(bridge_committee_config.bridge_authority_port_and_key_path)
                {
                    let account_kp = node_config.account_key_pair.keypair();
                    let iota_address = IotaAddress::from(&account_kp.public());
                    let gas_obj_ref = context
                        .get_one_gas_object_owned_by_address(iota_address)
                        .await?
                        .expect("Validator does not own any gas objects");
                    let kp = match read_key(&key_path, true)? {
                        IotaKeyPair::Secp256k1(key) => key,
                        _ => unreachable!("we required secp256k1 key in `read_key`"),
                    };

                    // build registration tx
                    let tx = build_committee_register_transaction(
                        iota_address,
                        &gas_obj_ref,
                        bridge_arg,
                        kp.public().as_bytes().to_vec(),
                        &format!("http://127.0.0.1:{port}"),
                        rgp,
                        1000000000,
                    )
                    .unwrap();
                    let signed_tx = context.sign_transaction(&tx);
                    tasks.push(context.execute_transaction_must_succeed(signed_tx));
                }
                futures::future::join_all(tasks).await;
                Ok(())
            }
            IotaCommand::FireDrill { fire_drill } => run_fire_drill(fire_drill).await,
            IotaCommand::Analyzer => {
                analyzer::run();
                Ok(())
            }
            #[cfg(feature = "gen-completions")]
            IotaCommand::GenerateCompletions(cmd) => cmd.run(),
        }
    }
}

/// Starts a local network with the given configuration.
async fn start(
    config_dir: Option<PathBuf>,
    with_faucet: Option<String>,
    faucet_amount: Option<u64>,
    #[cfg(feature = "indexer")] indexer_feature_args: IndexerFeatureArgs,
    force_regenesis: bool,
    epoch_duration_ms: Option<u64>,
    fullnode_rpc_port: u16,
    #[cfg(feature = "indexer")] mut data_ingestion_dir: Option<PathBuf>,
    no_full_node: bool,
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
    let config = config_dir.clone().map_or_else(iota_config_dir, Ok)?;
    let network_config_path = config.clone().join(IOTA_NETWORK_CONFIG);

    let mut swarm_builder = Swarm::builder();

    // If this is set, then no data will be persisted between runs, and a new
    // genesis will be generated each run.
    if force_regenesis {
        swarm_builder =
            swarm_builder.committee_size(NonZeroUsize::new(DEFAULT_NUMBER_OF_AUTHORITIES).unwrap());
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
        // Auto genesis if no configuration exists in the configuration directory.
        if !network_config_path.exists() {
            genesis(
                None,
                None,
                Some(config.clone()),
                false,
                epoch_duration_ms,
                None,
                false,
                DEFAULT_NUMBER_OF_AUTHORITIES,
                local_migration_snapshots,
                remote_migration_snapshots,
                delegator,
            )
            .await?;
        }

        let NetworkConfigLight {
            validator_configs,
            account_keys,
            ..
        } = PersistedConfig::read(&network_config_path).map_err(|err| {
            err.context(format!(
                "Cannot open IOTA network config file at {:?}",
                network_config_path
            ))
        })?;
        let genesis_path = config.join(IOTA_GENESIS_FILENAME);
        let genesis = iota_config::genesis::Genesis::load(genesis_path)?;
        let network_config = NetworkConfig {
            validator_configs,
            account_keys,
            genesis,
        };

        swarm_builder = swarm_builder
            .dir(config)
            .with_network_config(network_config);
    }

    // the indexer requires to set the fullnode's data ingestion directory
    // note that this overrides the default configuration that is set when running
    // the genesis command, which sets data_ingestion_dir to None.
    #[cfg(feature = "indexer")]
    if with_indexer.is_some() && data_ingestion_dir.is_none() {
        data_ingestion_dir = Some(tempdir()?.into_path())
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
    let fullnode_url = format!("http://{}", fullnode_url);
    info!("Fullnode URL: {}", fullnode_url);
    #[cfg(feature = "indexer")]
    let pg_address = format!("postgres://{pg_user}:{pg_password}@{pg_host}:{pg_port}/{pg_db_name}");

    #[cfg(feature = "indexer")]
    if let Some(input) = with_indexer {
        let indexer_address = parse_host_port(input, DEFAULT_INDEXER_PORT)
            .map_err(|_| anyhow!("Invalid indexer host and port"))?;
        tracing::info!("Starting the indexer service at {indexer_address}");
        // Start in writer mode
        start_test_indexer(
            Some(pg_address.clone()),
            fullnode_url.clone(),
            ReaderWriterConfig::writer_mode(None),
            data_ingestion_dir.clone(),
            None,
        )
        .await;
        info!("Indexer in writer mode started");

        // Start in reader mode
        start_test_indexer(
            Some(pg_address.clone()),
            fullnode_url.clone(),
            ReaderWriterConfig::reader_mode(indexer_address.to_string()),
            data_ingestion_dir,
            None,
        )
        .await;
        info!("Indexer in reader mode started");
    }

    #[cfg(feature = "indexer")]
    if let Some(input) = with_graphql {
        let graphql_address = parse_host_port(input, DEFAULT_GRAPHQL_PORT)
            .map_err(|_| anyhow!("Invalid graphql host and port"))?;
        tracing::info!("Starting the GraphQL service at {graphql_address}");
        let graphql_connection_config = ConnectionConfig::new(
            Some(graphql_address.port()),
            Some(graphql_address.ip().to_string()),
            Some(pg_address),
            None,
            None,
            None,
        );
        start_graphql_server_with_fn_rpc(
            graphql_connection_config,
            Some(fullnode_url.clone()),
            None, // it will be initialized by default
        )
        .await;
        info!("GraphQL started");
    }

    if let Some(input) = with_faucet {
        let faucet_address = parse_host_port(input, DEFAULT_FAUCET_PORT)
            .map_err(|_| anyhow!("Invalid faucet host and port"))?;
        tracing::info!("Starting the faucet service at {faucet_address}");
        let config_dir = if force_regenesis {
            tempdir()?.into_path()
        } else {
            match config_dir {
                Some(config) => config,
                None => iota_config_dir()?,
            }
        };

        let host_ip = match faucet_address {
            SocketAddr::V4(addr) => *addr.ip(),
            _ => bail!("faucet configuration requires an IPv4 address"),
        };

        let config = FaucetConfig {
            host_ip,
            port: faucet_address.port(),
            num_coins: DEFAULT_FAUCET_NUM_COINS,
            amount: faucet_amount.unwrap_or(DEFAULT_FAUCET_NANOS_AMOUNT),
            ..Default::default()
        };

        let prometheus_registry = prometheus::Registry::new();
        if force_regenesis {
            let kp = swarm.config_mut().account_keys.swap_remove(0);
            let keystore_path = config_dir.join(IOTA_KEYSTORE_FILENAME);
            let mut keystore = Keystore::from(FileBasedKeystore::new(&keystore_path).unwrap());
            let address: IotaAddress = kp.public().into();
            keystore.add_key(None, IotaKeyPair::Ed25519(kp)).unwrap();
            IotaClientConfig::new(keystore)
                .with_envs([IotaEnv::new("localnet", fullnode_url)])
                .with_active_address(address)
                .with_active_env("localnet".to_string())
                .persisted(config_dir.join(IOTA_CLIENT_CONFIG).as_path())
                .save()
                .unwrap();
        }
        let faucet_wal = config_dir.join("faucet.wal");
        let simple_faucet = SimpleFaucet::new(
            create_wallet_context(config.wallet_client_timeout_secs, config_dir)?,
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
    benchmark_ips: Option<Vec<String>>,
    with_faucet: bool,
    num_validators: usize,
    local_migration_snapshots: Vec<PathBuf>,
    remote_migration_snapshots: Vec<SnapshotUrl>,
    delegator: Option<IotaAddress>,
) -> Result<(), anyhow::Error> {
    let iota_config_dir = &match working_dir {
        // if a directory is specified, it must exist (it
        // will not be created)
        Some(v) => v,
        // create default IOTA config dir if not specified
        // on the command line and if it does not exist
        // yet
        None => {
            let config_path = iota_config_dir()?;
            fs::create_dir_all(&config_path)?;
            config_path
        }
    };

    // if IOTA config dir is not empty then either clean it
    // up (if --force/-f option was specified or report an
    // error
    let dir = iota_config_dir.read_dir().map_err(|err| {
        anyhow!(err).context(format!("Cannot open IOTA config dir {:?}", iota_config_dir))
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
                            anyhow!(err).context(format!("Cannot remove file {:?}", file.path()))
                        })?;
                    }
                }
            } else {
                fs::remove_dir_all(iota_config_dir).map_err(|err| {
                    anyhow!(err).context(format!(
                        "Cannot remove IOTA config dir {:?}",
                        iota_config_dir
                    ))
                })?;
                fs::create_dir(iota_config_dir).map_err(|err| {
                    anyhow!(err).context(format!(
                        "Cannot create IOTA config dir {:?}",
                        iota_config_dir
                    ))
                })?;
            }
        } else if files.len() != 2 || !client_path.exists() || !keystore_path.exists() {
            bail!(
                "cannot run genesis with non-empty IOTA config directory {}, please use the --force/-f option to remove the existing configuration",
                iota_config_dir.to_str().unwrap()
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
                for gas_key in GenesisConfig::benchmark_gas_keys(ips.len()) {
                    keystore.add_key(None, gas_key)?;
                }
                keystore.save()?;

                // Make a new genesis config from the provided ip addresses.
                GenesisConfig::new_for_benchmarks(&ips)
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
    let mut builder = ConfigBuilder::new(iota_config_dir)
        .with_genesis_config(genesis_conf)
        .with_empty_validator_genesis();
    builder = if let Some(validators) = validator_info {
        builder.with_validators(validators)
    } else {
        builder.committee_size(NonZeroUsize::new(num_validators).unwrap())
    };
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
        .with_config_directory(FULL_NODE_DB_PATH.into())
        .with_rpc_addr(iota_config::node::default_json_rpc_address())
        .with_genesis(genesis.clone())
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
                .with_config_directory(FULL_NODE_DB_PATH.into())
                .with_p2p_external_address(ssfn.p2p_address)
                .with_network_key_pair(ssfn.network_key_pair)
                .with_p2p_listen_address(([0, 0, 0, 0], 8084))
                .with_db_path(PathBuf::from("/opt/iota/db/authorities_db/full_node_db"))
                .with_network_address("/ip4/0.0.0.0/tcp/8080/http".parse()?)
                .with_metrics_address(([0, 0, 0, 0], 9184))
                .with_admin_interface_address(([127, 0, 0, 1], 1337))
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
        IotaClientConfig::new(keystore)
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
    client_config.add_env(IotaEnv::new(
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

async fn prompt_if_no_config(
    wallet_conf_path: &Path,
    accept_defaults: bool,
    generate_address: bool,
) -> anyhow::Result<()> {
    // Prompt user for connect to devnet fullnode if config does not exist.
    if !wallet_conf_path.exists() {
        let env = match std::env::var_os("IOTA_CONFIG_WITH_RPC_URL") {
            Some(v) => Some(IotaEnv::new("custom", v.into_string().unwrap())),
            None => {
                if accept_defaults {
                    print!(
                        "Creating config file [{:?}] with default (devnet) Full node server and ed25519 key scheme.",
                        wallet_conf_path
                    );
                } else {
                    print!(
                        "Config file [{:?}] doesn't exist, do you want to connect to an IOTA Full node server [y/N]?",
                        wallet_conf_path
                    );
                }
                if accept_defaults
                    || matches!(read_line(), Ok(line) if line.trim().to_lowercase() == "y")
                {
                    let url = if accept_defaults {
                        String::new()
                    } else {
                        print!(
                            "IOTA Full node server URL (Defaults to IOTA Testnet if not specified) : "
                        );
                        read_line()?
                    };
                    Some(if url.trim().is_empty() {
                        IotaEnv::testnet()
                    } else {
                        print!("Environment alias for [{url}] : ");
                        let alias = read_line()?;
                        let alias = if alias.trim().is_empty() {
                            "custom".to_string()
                        } else {
                            alias
                        };
                        IotaEnv::new(alias, url)
                    })
                } else {
                    None
                }
            }
        };

        if let Some(env) = env {
            let keystore_path = wallet_conf_path
                .parent()
                .unwrap_or(&iota_config_dir()?)
                .join(IOTA_KEYSTORE_FILENAME);
            let keystore = Keystore::from(FileBasedKeystore::new(&keystore_path)?);
            let mut config = IotaClientConfig::new(keystore).with_envs([env]);
            // Get an existing address or generate a new one
            if let Some(existing_address) = config.keystore().addresses().first() {
                println!("Using existing address {existing_address} as active address.");
                config = config.with_active_address(*existing_address);
            } else if generate_address {
                let key_scheme = if accept_defaults {
                    SignatureScheme::ED25519
                } else {
                    println!(
                        "Select key scheme to generate keypair (0 for ed25519, 1 for secp256k1, 2: for secp256r1):"
                    );
                    match SignatureScheme::from_flag(read_line()?.trim()) {
                        Ok(s) => s,
                        Err(e) => return Err(anyhow!("{e}")),
                    }
                };
                let (new_address, phrase, scheme) = config
                    .keystore_mut()
                    .generate_and_add_new_key(key_scheme, None, None, None)?;
                let alias = config.keystore().get_alias_by_address(&new_address)?;
                println!(
                    "Generated new keypair and alias for address with scheme {:?} [{alias}: {new_address}]",
                    scheme.to_string()
                );
                println!("Secret Recovery Phrase : [{phrase}]");
                config = config.with_active_address(new_address);
            }
            config.persisted(wallet_conf_path).save()?;
        }
    }
    Ok(())
}

fn read_line() -> Result<String, anyhow::Error> {
    let mut s = String::new();
    let _ = stdout().flush();
    io::stdin().read_line(&mut s)?;
    Ok(s.trim_end().to_string())
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
    } else if input.is_empty() {
        format!("{default_host}:{default_port_if_missing}").parse::<SocketAddr>()
    } else if !input.is_empty() {
        format!("{default_host}:{input}").parse::<SocketAddr>()
    } else {
        format!("{default_host}:{default_port_if_missing}").parse::<SocketAddr>()
    }
}
