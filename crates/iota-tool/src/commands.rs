// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeMap, env, num::NonZeroUsize, path::PathBuf, sync::Arc};

use anyhow::Result;
use clap::*;
use futures::{StreamExt, future::join_all};
use iota_config::{
    genesis::Genesis,
    object_storage_config::{ObjectStoreConfig, ObjectStoreType},
};
use iota_core::{
    authority_aggregator::AuthorityAggregatorBuilder,
    authority_client::{validator::ValidatorAPI, validator_peer::ValidatorPeerAPI},
};
use iota_replay::{ReplayToolCommand, execute_replay_command};
use iota_sdk::{IotaClient, IotaClientBuilder, rpc_types::IotaTransactionBlockResponseOptions};
use iota_sdk_types::{Address, ObjectId, SenderSignedTransaction, TransactionDigest};
use iota_snapshot::progress::LOG_TARGET_PROGRESS;
use iota_types::{
    base_types::*,
    crypto::AuthorityPublicKeyBytes,
    messages_checkpoint::{CheckpointRequest, CheckpointResponse, CheckpointSequenceNumber},
    messages_grpc::TransactionInfoRequest,
    transaction::TransactionEnvelope,
};
use telemetry_subscribers::TracingHandle;

use crate::{
    ConciseObjectOutput, GroupedObjectOutput, SnapshotVerifyMode, VerboseObjectOutput,
    backfill_checkpoint_summaries, check_completed_snapshot,
    db_tool::{DbToolCommand, execute_db_tool_command, print_db_all_tables},
    download_formal_snapshot, get_latest_available_epoch, get_object, get_transaction_block,
    make_clients,
};

/// Log filter for the restore commands' non-verbose default: silence
/// everything except the progress status lines, which are the only progress
/// output left once the progress bars can't be drawn.
fn progress_only_log_directives() -> String {
    format!("off,{LOG_TARGET_PROGRESS}=info")
}

#[derive(Parser, Clone, ValueEnum)]
pub enum Verbosity {
    Grouped,
    Concise,
    Verbose,
}

/// Networks that publish snapshots downloadable with this tool.
#[derive(Debug, Copy, Clone, ValueEnum)]
pub enum Network {
    Mainnet,
    Testnet,
    Devnet,
}

#[derive(Parser)]
pub enum ToolCommand {
    /// Inspect if a specific object is or all gas objects owned by an address
    /// are locked by validators
    LockedObject {
        /// Either id or address must be provided
        /// The object to check
        #[arg(long, help = "The object ID to fetch")]
        id: Option<ObjectId>,
        /// Either id or address must be provided
        /// If provided, check all gas objects owned by this account
        #[arg(long)]
        address: Option<Address>,
        /// RPC address to provide the up-to-date committee info
        #[arg(long)]
        fullnode_rpc_url: String,
        /// Should attempt to rescue the object if it's locked but not fully
        /// locked
        #[arg(long)]
        rescue: bool,
    },

    /// Fetch the same object from all validators
    FetchObject {
        #[arg(long, help = "The object ID to fetch")]
        id: ObjectId,

        #[arg(long, help = "Fetch object at a specific sequence")]
        version: Option<u64>,

        #[arg(
            long,
            help = "Validator to fetch from - if not specified, all validators are queried"
        )]
        validator: Option<AuthorityName>,

        // RPC address to provide the up-to-date committee info
        #[arg(long)]
        fullnode_rpc_url: String,

        /// Concise mode groups responses by results.
        /// prints tabular output suitable for processing with unix tools. For
        /// instance, to quickly check that all validators agree on the history
        /// of an object: ```text
        /// $ iota-tool fetch-object --id
        /// 0x260efde76ebccf57f4c5e951157f5c361cde822c \      --genesis
        /// $HOME/.iota/iota_config/genesis.blob \      --verbosity
        /// concise --concise-no-header ```
        #[arg(value_enum, long, default_value = "grouped", ignore_case = true)]
        verbosity: Verbosity,

        #[arg(long, help = "don't show header in concise output")]
        concise_no_header: bool,
    },

    /// Fetch the effects association with transaction `digest`
    FetchTransaction {
        // RPC address to provide the up-to-date committee info
        #[arg(long)]
        fullnode_rpc_url: String,

        #[arg(long, help = "The transaction ID to fetch")]
        digest: TransactionDigest,

        /// If true, show the input transaction as well as the effects
        #[arg(long = "show-tx")]
        show_input_tx: bool,
    },

    /// Tool to read validator & node db.
    DbTool {
        /// Path of the DB to read
        #[arg(long)]
        db_path: String,
        #[command(subcommand)]
        cmd: Option<DbToolCommand>,
    },

    /// Download all packages to the local filesystem from a GraphQL service.
    /// Each package gets its own sub-directory, named for its ID on chain
    /// and version containing two metadata files (linkage.json and
    /// origins.json), a file containing the overall object and a file for every
    /// module it contains. Each module file is named for its module name, with
    /// a .mv suffix, and contains Move bytecode (suitable for passing into
    /// a disassembler).
    DumpPackages {
        /// Connection information for a GraphQL service.
        #[arg(long, short)]
        rpc_url: String,

        /// Path to a non-existent directory that can be created and filled with
        /// package information.
        #[arg(long, short)]
        output_dir: PathBuf,

        /// Only fetch packages that were created before this checkpoint (given
        /// by its sequence number).
        #[arg(long)]
        before_checkpoint: Option<u64>,

        /// If false (default), log level will be overridden to "off", and
        /// output will be reduced to necessary status information.
        #[arg(short, long)]
        verbose: bool,
    },

    DumpValidators {
        #[arg(long)]
        genesis: PathBuf,

        #[arg(
            long,
            help = "show concise output - name, authority key and network address"
        )]
        concise: bool,
    },

    DumpGenesis {
        #[arg(long)]
        genesis: PathBuf,
    },

    /// Fetch authenticated checkpoint information at a specific sequence
    /// number. If sequence number is not specified, get the latest
    /// authenticated checkpoint.
    FetchCheckpoint {
        // RPC address to provide the up-to-date committee info
        #[arg(long)]
        fullnode_rpc_url: String,

        #[arg(long, help = "Fetch checkpoint at a specific sequence number")]
        sequence_number: Option<CheckpointSequenceNumber>,
    },

    Anemo {
        #[command(next_help_heading = "foo", flatten)]
        args: anemo_cli::Args,
    },

    // Restore from formal (slim, DB agnostic) snapshot.
    #[command(
        about = "Downloads formal database snapshot via cloud object store, outputs to local disk"
    )]
    DownloadFormalSnapshot {
        /// Epoch to restore to the end of. Mutually exclusive with `--latest`.
        #[arg(long, conflicts_with = "latest")]
        epoch: Option<u64>,
        /// Path to the network's `genesis.blob`.
        #[arg(long)]
        genesis: PathBuf,
        /// Directory to restore into. The restored database is written to a
        /// `live` subdirectory of this path.
        #[arg(long)]
        path: PathBuf,
        /// Number of parallel downloads to perform. Defaults to logical cores -
        /// 1, capped at 8.
        #[arg(long)]
        num_parallel_downloads: Option<NonZeroUsize>,
        /// Verification mode to employ.
        #[arg(long, default_value = "normal")]
        verify: Option<SnapshotVerifyMode>,
        /// Network to download snapshot for. Defaults to "mainnet".
        /// If `--snapshot-bucket` is not specified, the value of this flag is
        /// used to construct the default bucket name.
        #[arg(long, default_value = "mainnet")]
        network: Network,
        /// Snapshot bucket name. If not specified, defaults are
        /// based on value of `--network` flag.
        #[arg(long, conflicts_with = "no_sign_request")]
        snapshot_bucket: Option<String>,
        /// Snapshot bucket type
        #[arg(
            long,
            conflicts_with = "no_sign_request",
            help = "Required if --no-sign-request is not set"
        )]
        snapshot_bucket_type: Option<ObjectStoreType>,
        /// Path to snapshot directory on local filesystem.
        /// Only applicable if `--snapshot-bucket-type` is "file".
        #[arg(long)]
        snapshot_path: Option<PathBuf>,
        /// If true, no authentication is needed for snapshot restores
        #[arg(
            long,
            conflicts_with_all = &["snapshot_bucket", "snapshot_bucket_type"],
            help = "if set, no authentication is needed for snapshot restore"
        )]
        no_sign_request: bool,
        /// Download snapshot of the latest available epoch.
        /// If `--epoch` is specified, then this flag gets ignored.
        #[arg(
            long,
            conflicts_with = "epoch",
            help = "defaults to latest available snapshot in chosen bucket"
        )]
        latest: bool,
        /// If false (default), log level will be overridden to "off",
        /// and output will be reduced to necessary status information.
        #[arg(long)]
        verbose: bool,

        /// Report progress as a status line logged once per second.
        #[arg(long)]
        disable_progress_bar: bool,

        /// Skip building the gRPC index store during the restore. By default
        /// it is built from the same object stream that restores the state,
        /// so a fullnode started with gRPC enabled opens it in place instead
        /// of re-indexing the whole restored state on first start.
        #[arg(long)]
        skip_grpc_indexes: bool,

        /// Skip building the JSON-RPC index store during the restore. By
        /// default it is built from the restored live object set, so a
        /// fullnode started with `enable-index-processing` opens it in place
        /// instead of re-indexing the whole restored state on first start.
        #[arg(long)]
        skip_jsonrpc_indexes: bool,
    },

    /// Backfill the full checkpoint summary history from the checkpoint
    /// archive into a stopped node's checkpoint store.
    ///
    /// A node restored from a formal snapshot holds only the end-of-epoch
    /// summaries. This downloads every intermediate summary up to the node's
    /// highest synced checkpoint, so the node holds the complete header chain
    /// from genesis (to serve historical checkpoint queries, or to be a full
    /// summary source for syncing peers). Only historical summaries are added;
    /// no watermark is moved.
    ///
    /// Summaries are downloaded from the checkpoint archive at
    /// `--ingestion-url` and inserted without chain verification, so the
    /// checkpoint archive is trusted to serve this node's own chain.
    BackfillCheckpointSummaries {
        /// Path to the node's live database directory (the one containing
        /// `checkpoints/`, `store/`, and `epochs/`). The node must be stopped.
        #[arg(long)]
        path: PathBuf,
        /// URL of the checkpoint archive to download summaries from (the same
        /// store a node's state sync reads from, e.g. an S3/GCS bucket or HTTP
        /// endpoint).
        #[arg(long)]
        ingestion_url: String,
        /// Number of parallel downloads to perform. Defaults to logical cores -
        /// 1, capped at 8.
        #[arg(long)]
        num_parallel_downloads: Option<NonZeroUsize>,
        /// If false (default), log level will be overridden to "off", and
        /// output will be reduced to necessary status information.
        #[arg(long)]
        verbose: bool,
        /// Report progress as a status line logged once per second.
        #[arg(long)]
        disable_progress_bar: bool,
    },

    Replay {
        #[arg(long = "rpc")]
        rpc_url: Option<String>,
        #[arg(long)]
        safety_checks: bool,
        #[arg(long = "authority")]
        use_authority: bool,
        #[arg(
            long,
            short,
            help = "Path to the network config file. This should be specified when rpc_url is not present. \
            If not specified we will use the default network config file at ~/.iota-replay/network-config.yaml"
        )]
        cfg_path: Option<PathBuf>,
        #[arg(
            long,
            help = "The name of the chain to replay from, could be one of: mainnet, testnet, devnet.\
            When rpc_url is not specified, this is used to load the corresponding config from the network config file.\
            If not specified, mainnet will be used by default"
        )]
        chain: Option<String>,
        #[command(subcommand)]
        cmd: ReplayToolCommand,
    },

    /// Ask all validators to sign a transaction through AuthorityAggregator.
    SignTransaction {
        #[arg(long)]
        genesis: PathBuf,

        #[arg(
            long,
            help = "The Base64-encoding of the bcs bytes of SenderSignedTransaction"
        )]
        sender_signed_data: String,
    },

    /// Create an IOTA Genesis Ceremony with multiple remote validators.
    GenesisCeremony(crate::genesis_ceremony::Ceremony),
    /// Tool for Fire Drill
    FireDrill {
        #[command(subcommand)]
        fire_drill: crate::fire_drill::FireDrill,
    },

    /// Check the health of a running gRPC server.
    /// Exits with code 0 if healthy, non-zero otherwise.
    #[command(name = "grpc-health-check")]
    GrpcHealthCheck {
        /// The gRPC server address (e.g., "http://localhost:50051")
        #[arg(long, default_value = "http://localhost:50051")]
        address: String,
    },
}

async fn check_locked_object(
    iota_client: &Arc<IotaClient>,
    committee: Arc<BTreeMap<AuthorityPublicKeyBytes, u64>>,
    id: ObjectId,
    rescue: bool,
) -> anyhow::Result<()> {
    let clients = Arc::new(make_clients(iota_client).await?);
    let output = get_object(id, None, None, clients.clone()).await?;
    let output = GroupedObjectOutput::new(output, committee);
    if output.fully_locked {
        println!("Object {id} is fully locked.");
        return Ok(());
    }
    let top_record = output.voting_power.first().unwrap();
    let top_record_stake = top_record.1;
    let top_record = top_record.0.unwrap();
    if top_record.4.is_none() {
        println!(
            "Object {id} does not seem to be locked by majority of validators (unlocked stake: {top_record_stake})"
        );
        return Ok(());
    }

    let tx_digest = top_record.2;
    if !rescue {
        println!("Object {id} is rescueable, top tx: {tx_digest}");
        return Ok(());
    }
    println!("Object {id} is rescueable, trying tx {tx_digest}");
    let validator = output
        .grouped_results
        .get(&Some(top_record))
        .unwrap()
        .first()
        .unwrap();
    let client = &clients.get(validator).unwrap().1;
    let tx = client
        .handle_transaction_info_request(TransactionInfoRequest {
            transaction_digest: tx_digest,
        })
        .await?
        .transaction;
    let res = iota_client
        .quorum_driver_api()
        .execute_transaction_block(
            TransactionEnvelope::new(tx),
            IotaTransactionBlockResponseOptions::full_content(),
            None,
        )
        .await;
    match res {
        Ok(_) => {
            println!("Transaction executed successfully ({tx_digest})");
        }
        Err(e) => {
            println!("Failed to execute transaction ({tx_digest}): {e:?}");
        }
    }
    Ok(())
}

impl ToolCommand {
    pub async fn execute(self, tracing_handle: TracingHandle) -> Result<(), anyhow::Error> {
        match self {
            ToolCommand::LockedObject {
                id,
                fullnode_rpc_url,
                rescue,
                address,
            } => {
                let iota_client =
                    Arc::new(IotaClientBuilder::default().build(fullnode_rpc_url).await?);
                let committee = Arc::new(
                    iota_client
                        .governance_api()
                        .get_committee_info(None)
                        .await?
                        .validators
                        .into_iter()
                        .collect::<BTreeMap<_, _>>(),
                );
                let object_ids = match id {
                    Some(id) => vec![id],
                    None => {
                        let address = address.expect("Either id or address must be provided");
                        iota_client
                            .coin_read_api()
                            .get_coins_stream(address, None)
                            .map(|c| c.coin_object_id)
                            .collect()
                            .await
                    }
                };
                for ids in object_ids.chunks(30) {
                    let mut tasks = vec![];
                    for id in ids {
                        tasks.push(check_locked_object(
                            &iota_client,
                            committee.clone(),
                            *id,
                            rescue,
                        ))
                    }
                    join_all(tasks)
                        .await
                        .into_iter()
                        .collect::<Result<Vec<_>, _>>()?;
                }
            }
            ToolCommand::FetchObject {
                id,
                validator,
                version,
                fullnode_rpc_url,
                verbosity,
                concise_no_header,
            } => {
                let iota_client =
                    Arc::new(IotaClientBuilder::default().build(fullnode_rpc_url).await?);
                let clients = Arc::new(make_clients(&iota_client).await?);
                let output = get_object(id, version, validator, clients).await?;

                match verbosity {
                    Verbosity::Grouped => {
                        let committee = Arc::new(
                            iota_client
                                .governance_api()
                                .get_committee_info(None)
                                .await?
                                .validators
                                .into_iter()
                                .collect::<BTreeMap<_, _>>(),
                        );
                        println!("{}", GroupedObjectOutput::new(output, committee));
                    }
                    Verbosity::Verbose => {
                        println!("{}", VerboseObjectOutput(output));
                    }
                    Verbosity::Concise => {
                        if !concise_no_header {
                            println!("{}", ConciseObjectOutput::header());
                        }
                        println!("{}", ConciseObjectOutput(output));
                    }
                }
            }
            ToolCommand::FetchTransaction {
                digest,
                show_input_tx,
                fullnode_rpc_url,
            } => {
                print!(
                    "{}",
                    get_transaction_block(digest, show_input_tx, fullnode_rpc_url).await?
                );
            }
            ToolCommand::DbTool { db_path, cmd } => {
                let path = PathBuf::from(db_path);
                match cmd {
                    Some(c) => execute_db_tool_command(path, c).await?,
                    None => print_db_all_tables(path)?,
                }
            }
            ToolCommand::DumpPackages {
                rpc_url,
                output_dir,
                before_checkpoint,
                verbose,
            } => {
                if !verbose {
                    tracing_handle
                        .update_log("off")
                        .expect("Failed to update log level");
                }

                iota_package_dump::dump(rpc_url, output_dir, before_checkpoint).await?;
            }
            ToolCommand::DumpValidators { genesis, concise } => {
                let genesis = Genesis::load(genesis).unwrap();
                if !concise {
                    println!("{:#?}", genesis.validator_set_for_tooling());
                } else {
                    for (i, val_info) in genesis.validator_set_for_tooling().iter().enumerate() {
                        let metadata = val_info.verified_metadata();
                        println!(
                            "#{:<2} {:<20} {:?} {:?} {}",
                            i,
                            metadata.name,
                            metadata.iota_pubkey_bytes().concise(),
                            metadata.net_address,
                            anemo::PeerId(metadata.network_pubkey.0.to_bytes()),
                        )
                    }
                }
            }
            ToolCommand::DumpGenesis { genesis } => {
                let genesis = Genesis::load(genesis)?;
                println!("{genesis:#?}");
            }
            ToolCommand::FetchCheckpoint {
                sequence_number,
                fullnode_rpc_url,
            } => {
                let iota_client =
                    Arc::new(IotaClientBuilder::default().build(fullnode_rpc_url).await?);
                let clients = make_clients(&iota_client).await?;

                for (name, (_, client)) in clients {
                    let resp = client
                        .get_checkpoint_v2(CheckpointRequest {
                            sequence_number,
                            request_content: true,
                            certified: true,
                        })
                        .await
                        .unwrap();
                    let CheckpointResponse {
                        checkpoint,
                        contents,
                    } = resp;
                    println!("Validator: {:?}\n", name.concise());
                    println!("Checkpoint: {checkpoint:?}\n");
                    println!("Content: {contents:?}\n");
                }
            }
            ToolCommand::Anemo { args } => {
                let config = crate::make_anemo_config();
                anemo_cli::run(config, args).await
            }
            ToolCommand::DownloadFormalSnapshot {
                epoch,
                genesis,
                path,
                num_parallel_downloads,
                verify,
                network,
                snapshot_bucket,
                snapshot_bucket_type,
                snapshot_path,
                no_sign_request,
                latest,
                verbose,
                disable_progress_bar,
                skip_grpc_indexes,
                skip_jsonrpc_indexes,
            } => {
                if !verbose {
                    tracing_handle
                        .update_log(progress_only_log_directives())
                        .expect("Failed to update log level");
                }
                let num_parallel_downloads = num_parallel_downloads
                    .unwrap_or_else(iota_snapshot::default_download_concurrency);
                let snapshot_bucket =
                    snapshot_bucket.or_else(|| match (network, no_sign_request) {
                        (Network::Mainnet, false) => Some(
                            env::var("MAINNET_FORMAL_SIGNED_BUCKET")
                                .unwrap_or("iota-mainnet-formal".to_string()),
                        ),
                        (Network::Mainnet, true) => env::var("MAINNET_FORMAL_UNSIGNED_BUCKET").ok(),
                        (Network::Testnet, false) => Some(
                            env::var("TESTNET_FORMAL_SIGNED_BUCKET")
                                .unwrap_or("iota-testnet-formal".to_string()),
                        ),
                        (Network::Testnet, true) => env::var("TESTNET_FORMAL_UNSIGNED_BUCKET").ok(),
                        (Network::Devnet, false) => Some(
                            env::var("DEVNET_FORMAL_SIGNED_BUCKET")
                                .unwrap_or("iota-devnet-formal".to_string()),
                        ),
                        (Network::Devnet, true) => env::var("DEVNET_FORMAL_UNSIGNED_BUCKET").ok(),
                    });

                let aws_endpoint = env::var("AWS_SNAPSHOT_ENDPOINT").ok().or_else(|| {
                    no_sign_request.then(|| {
                        match network {
                            Network::Mainnet => "https://formal-snapshot.mainnet.iota.cafe",
                            Network::Testnet => "https://formal-snapshot.testnet.iota.cafe",
                            Network::Devnet => "https://formal-snapshot.devnet.iota.cafe",
                        }
                        .to_string()
                    })
                });

                let snapshot_bucket_type = if no_sign_request {
                    ObjectStoreType::S3
                } else {
                    snapshot_bucket_type
                        .expect("You must set either --snapshot-bucket-type or --no-sign-request")
                };
                let snapshot_store_config = match snapshot_bucket_type {
                    ObjectStoreType::S3 => ObjectStoreConfig {
                        object_store: Some(ObjectStoreType::S3),
                        bucket: snapshot_bucket.filter(|s| !s.is_empty()),
                        aws_access_key_id: env::var("AWS_SNAPSHOT_ACCESS_KEY_ID").ok(),
                        aws_secret_access_key: env::var("AWS_SNAPSHOT_SECRET_ACCESS_KEY").ok(),
                        aws_region: env::var("AWS_SNAPSHOT_REGION").ok(),
                        aws_endpoint: aws_endpoint.filter(|s| !s.is_empty()),
                        aws_virtual_hosted_style_request: env::var(
                            "AWS_SNAPSHOT_VIRTUAL_HOSTED_REQUESTS",
                        )
                        .ok()
                        .and_then(|b| b.parse().ok())
                        .unwrap_or(no_sign_request),
                        object_store_connection_limit: 200,
                        no_sign_request,
                        ..Default::default()
                    },
                    ObjectStoreType::GCS => ObjectStoreConfig {
                        object_store: Some(ObjectStoreType::GCS),
                        bucket: snapshot_bucket,
                        google_service_account: env::var("GCS_SNAPSHOT_SERVICE_ACCOUNT_FILE_PATH")
                            .ok(),
                        object_store_connection_limit: 200,
                        no_sign_request,
                        ..Default::default()
                    },
                    ObjectStoreType::Azure => ObjectStoreConfig {
                        object_store: Some(ObjectStoreType::Azure),
                        bucket: snapshot_bucket,
                        azure_storage_account: env::var("AZURE_SNAPSHOT_STORAGE_ACCOUNT").ok(),
                        azure_storage_access_key: env::var("AZURE_SNAPSHOT_STORAGE_ACCESS_KEY")
                            .ok(),
                        object_store_connection_limit: 200,
                        no_sign_request,
                        ..Default::default()
                    },
                    ObjectStoreType::File => {
                        if snapshot_path.is_some() {
                            ObjectStoreConfig {
                                object_store: Some(ObjectStoreType::File),
                                directory: snapshot_path,
                                ..Default::default()
                            }
                        } else {
                            panic!(
                                "--snapshot-path must be specified for --snapshot-bucket-type=file"
                            );
                        }
                    }
                };

                let latest_available_epoch =
                    latest.then_some(get_latest_available_epoch(&snapshot_store_config).await?);
                let epoch_to_download = epoch.or(latest_available_epoch).expect(
                    "Either pass epoch with --epoch <epoch_num> or use latest with --latest",
                );

                if let Err(e) =
                    check_completed_snapshot(&snapshot_store_config, epoch_to_download).await
                {
                    panic!("Aborting snapshot restore: {e}, snapshot may not be uploaded yet");
                }

                let verify = verify.unwrap_or_default();
                download_formal_snapshot(
                    &path,
                    epoch_to_download,
                    &genesis,
                    snapshot_store_config,
                    num_parallel_downloads,
                    verify,
                    skip_grpc_indexes,
                    skip_jsonrpc_indexes,
                    disable_progress_bar,
                )
                .await?;
            }
            ToolCommand::BackfillCheckpointSummaries {
                path,
                ingestion_url,
                num_parallel_downloads,
                verbose,
                disable_progress_bar,
            } => {
                if !verbose {
                    tracing_handle
                        .update_log(progress_only_log_directives())
                        .expect("Failed to update log level");
                }
                let num_parallel_downloads = num_parallel_downloads
                    .unwrap_or_else(iota_snapshot::default_download_concurrency);
                backfill_checkpoint_summaries(
                    &path,
                    ingestion_url,
                    num_parallel_downloads,
                    disable_progress_bar,
                )
                .await?;
            }
            ToolCommand::Replay {
                rpc_url,
                safety_checks,
                cmd,
                use_authority,
                cfg_path,
                chain,
            } => {
                execute_replay_command(rpc_url, safety_checks, use_authority, cfg_path, chain, cmd)
                    .await?;
            }
            ToolCommand::SignTransaction {
                genesis,
                sender_signed_data,
            } => {
                let genesis = Genesis::load(genesis)?;
                let sender_signed_tx =
                    SenderSignedTransaction::from_base64(sender_signed_data.as_str()).unwrap();
                let transaction = TransactionEnvelope::new(sender_signed_tx);
                let (agg, _) =
                    AuthorityAggregatorBuilder::from_genesis(&genesis).build_network_clients();
                let result = agg.process_transaction(transaction, None).await;
                println!("{result:?}");
            }
            ToolCommand::GenesisCeremony(cmd) => {
                crate::genesis_ceremony::run(cmd).await?;
            }
            ToolCommand::FireDrill { fire_drill } => {
                crate::fire_drill::run_fire_drill(fire_drill).await?;
            }
            ToolCommand::GrpcHealthCheck { address } => {
                let client = iota_grpc_client::Client::new(address)?;
                client.get_health(None).await?;
                println!("OK");
            }
        };
        Ok(())
    }
}
