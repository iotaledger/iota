// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{cmp::max, env, io::BufRead, path::PathBuf, str::FromStr};

use async_recursion::async_recursion;
use clap::Parser;
use config::ReplayableNetworkConfigSet;
use fuzz::{ReplayFuzzer, ReplayFuzzerConfig};
use fuzz_mutations::base_fuzzers;
use iota_config::node::ExpensiveSafetyCheckConfig;
use iota_protocol_config::Chain;
use iota_types::{
    base_types::{ObjectID, SequenceNumber},
    digests::{get_mainnet_chain_identifier, get_testnet_chain_identifier, TransactionDigest},
    message_envelope::Message,
};
use move_vm_config::runtime::get_default_output_filepath;
use tracing::{error, info, warn};
use transaction_provider::{FuzzStartPoint, TransactionSource};

use crate::{
    config::get_rpc_url,
    replay::{ExecutionSandboxState, LocalExec, ProtocolVersionSummary},
};

pub mod batch_replay;
pub mod config;
mod data_fetcher;
mod displays;
pub mod fuzz;
pub mod fuzz_mutations;
mod replay;
#[cfg(test)]
mod tests;
pub mod transaction_provider;
pub mod types;

static DEFAULT_SANDBOX_BASE_PATH: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/tests/sandbox_snapshots");

#[derive(Parser, Clone)]
#[command(rename_all = "kebab-case")]
pub enum ReplayToolCommand {
    /// Generate a new network config file
    #[command(name = "gen")]
    GenerateDefaultConfig,

    /// Persist sandbox state
    #[command(name = "ps")]
    PersistSandbox {
        #[arg(long, short)]
        tx_digest: String,
        #[arg(long, short, default_value = DEFAULT_SANDBOX_BASE_PATH)]
        base_path: PathBuf,
    },

    /// Replay from sandbox state file
    /// This is a completely local execution
    #[command(name = "rs")]
    ReplaySandbox {
        #[arg(long, short)]
        path: PathBuf,
    },

    /// Profile transaction
    #[command(name = "rp")]
    ProfileTransaction {
        #[arg(long, short)]
        tx_digest: String,
        /// Optional version of the executor to use, if not specified defaults
        /// to the one originally used for the transaction.
        #[arg(long, short, allow_hyphen_values = true)]
        executor_version: Option<i64>,
        /// Optional protocol version to use, if not specified defaults to the
        /// one originally used for the transaction.
        #[arg(long, short, allow_hyphen_values = true)]
        protocol_version: Option<i64>,
        /// Optional output filepath for the profile generated by this run, if
        /// not specified defaults to
        /// `gas_profile_{tx_digest}_{unix_timestamp}.json in the working
        /// directory.
        #[arg(long, short, allow_hyphen_values = true)]
        profile_output: Option<PathBuf>,
        /// Required config objects and versions of the config objects to use if
        /// replaying a transaction that utilizes the config object for
        /// regulated coin types and that has been denied.
        #[arg(long, num_args = 2..)]
        config_objects: Option<Vec<String>>,
    },

    /// Replay transaction
    #[command(name = "tx")]
    ReplayTransaction {
        #[arg(long, short)]
        tx_digest: String,
        #[arg(long, short)]
        show_effects: bool,
        /// Optional version of the executor to use, if not specified defaults
        /// to the one originally used for the transaction.
        #[arg(long, short, allow_hyphen_values = true)]
        executor_version: Option<i64>,
        /// Optional protocol version to use, if not specified defaults to the
        /// one originally used for the transaction.
        #[arg(long, short, allow_hyphen_values = true)]
        protocol_version: Option<i64>,
        /// Required config objects and versions of the config objects to use if
        /// replaying a transaction that utilizes the config object for
        /// regulated coin types and that has been denied.
        #[arg(long, num_args = 2..)]
        config_objects: Option<Vec<String>>,
    },

    /// Replay transactions listed in a file
    #[command(name = "rb")]
    ReplayBatch {
        #[arg(long, short)]
        path: PathBuf,
        #[arg(long, short)]
        terminate_early: bool,
        #[arg(
            long,
            short,
            default_value = "16",
            help = "Number of tasks to run in parallel"
        )]
        num_tasks: u64,
        #[arg(
            long,
            help = "If provided, dump the state of the execution to a file in the given directory. \
            This will allow faster replay next time."
        )]
        persist_path: Option<PathBuf>,
    },

    /// Replay a transaction from a node state dump
    #[command(name = "rd")]
    ReplayDump {
        #[arg(long, short)]
        path: String,
        #[arg(long, short)]
        show_effects: bool,
    },

    /// Replay multiple transactions from JSON files that contain the sandbox
    /// persisted state.
    #[command(name = "brd")]
    BatchReplayFromSandbox {
        #[arg(
            help = "The path to the directory that contains many JSON files, each representing a persisted sandbox.\
            These files are typically generated by running the ReplayBatch command with --persist-path specified."
        )]
        path: String,
        #[arg(
            long,
            short,
            default_value = "64",
            help = "Number of tasks to run in parallel"
        )]
        num_tasks: usize,
    },

    /// Replay all transactions in a range of checkpoints
    #[command(name = "ch")]
    ReplayCheckpoints {
        #[arg(long, short)]
        start: u64,
        #[arg(long, short)]
        end: u64,
        #[arg(long, short)]
        terminate_early: bool,
        #[arg(long, short, default_value = "16")]
        max_tasks: u64,
    },

    /// Replay all transactions in an epoch
    #[command(name = "ep")]
    ReplayEpoch {
        #[arg(long, short)]
        epoch: u64,
        #[arg(long, short)]
        terminate_early: bool,
        #[arg(long, short, default_value = "16")]
        max_tasks: u64,
    },

    /// Run the replay based fuzzer
    #[command(name = "fz")]
    Fuzz {
        #[arg(long, short)]
        start: Option<FuzzStartPoint>,
        #[arg(long, short)]
        num_mutations_per_base: u64,
        #[arg(long, short = 'b', default_value = "18446744073709551614")]
        num_base_transactions: u64,
    },

    #[command(name = "report")]
    Report,
}

#[async_recursion]
pub async fn execute_replay_command(
    rpc_url: Option<String>,
    safety_checks: bool,
    use_authority: bool,
    cfg_path: Option<PathBuf>,
    chain: Option<String>,
    cmd: ReplayToolCommand,
) -> anyhow::Result<Option<(u64, u64)>> {
    let safety = if safety_checks {
        ExpensiveSafetyCheckConfig::new_enable_all()
    } else {
        ExpensiveSafetyCheckConfig::default()
    };
    Ok(match cmd {
        ReplayToolCommand::ReplaySandbox { path } => {
            let contents = std::fs::read_to_string(path)?;
            let sandbox_state: ExecutionSandboxState = serde_json::from_str(&contents)?;
            info!("Executing tx: {}", sandbox_state.transaction_info.tx_digest);
            let sandbox_state =
                LocalExec::certificate_execute_with_sandbox_state(&sandbox_state).await?;
            sandbox_state.check_effects()?;
            info!("Execution finished successfully. Local and on-chain effects match.");
            None
        }
        ReplayToolCommand::PersistSandbox {
            tx_digest,
            base_path,
        } => {
            let tx_digest = TransactionDigest::from_str(&tx_digest)?;
            info!("Executing tx: {}", tx_digest);
            let sandbox_state = LocalExec::replay_with_network_config(
                get_rpc_url(rpc_url, cfg_path, chain)?,
                tx_digest,
                safety,
                use_authority,
                None,
                None,
                None,
                None,
            )
            .await?;

            let out = serde_json::to_string(&sandbox_state).unwrap();
            let path = base_path.join(format!("{}.json", tx_digest));
            std::fs::write(path, out)?;
            None
        }
        ReplayToolCommand::GenerateDefaultConfig => {
            let set = ReplayableNetworkConfigSet::default();
            let path = set.save_config(None).unwrap();
            println!("Default config saved to: {}", path.to_str().unwrap());
            warn!("Note: default config nodes might prune epochs/objects");
            None
        }
        ReplayToolCommand::Fuzz {
            start,
            num_mutations_per_base,
            num_base_transactions,
        } => {
            let config = ReplayFuzzerConfig {
                num_mutations_per_base,
                mutator: Box::new(base_fuzzers(num_mutations_per_base)),
                tx_source: TransactionSource::TailLatest { start },
                fail_over_on_err: false,
                expensive_safety_check_config: Default::default(),
            };
            let fuzzer = ReplayFuzzer::new(get_rpc_url(rpc_url, cfg_path, chain)?, config)
                .await
                .unwrap();
            fuzzer.run(num_base_transactions).await.unwrap();
            None
        }
        ReplayToolCommand::ReplayDump { path, show_effects } => {
            let mut lx = LocalExec::new_for_state_dump(&path, rpc_url).await?;
            let (sandbox_state, node_dump_state) = lx.execute_state_dump(safety).await?;
            if show_effects {
                println!("{:#?}", sandbox_state.local_exec_effects);
            }

            sandbox_state.check_effects()?;

            let effects = node_dump_state.computed_effects.digest();
            if effects != node_dump_state.expected_effects_digest {
                error!(
                    "Effects digest mismatch for {}: expected: {:?}, got: {:?}",
                    node_dump_state.tx_digest, node_dump_state.expected_effects_digest, effects,
                );
                anyhow::bail!("Effects mismatch");
            }

            info!("Execution finished successfully. Local and on-chain effects match.");
            Some((1u64, 1u64))
        }
        ReplayToolCommand::ReplayBatch {
            path,
            terminate_early,
            num_tasks,
            persist_path,
        } => {
            let file = std::fs::File::open(path).unwrap();
            let buf_reader = std::io::BufReader::new(file);
            let digests = buf_reader.lines().map(|line| {
                let line = line.unwrap();
                TransactionDigest::from_str(&line).unwrap_or_else(|err| {
                    panic!("Error parsing tx digest {:?}: {:?}", line, err);
                })
            });
            batch_replay::batch_replay(
                digests,
                num_tasks,
                get_rpc_url(rpc_url, cfg_path, chain)?,
                safety,
                use_authority,
                terminate_early,
                persist_path,
            )
            .await;

            // TODO: clean this up
            Some((0u64, 0u64))
        }
        ReplayToolCommand::BatchReplayFromSandbox { path, num_tasks } => {
            let files: Vec<_> = std::fs::read_dir(path)?
                .filter_map(|entry| {
                    let path = entry.ok()?.path();
                    if path.is_file() {
                        path.to_str().map(|p| p.to_owned())
                    } else {
                        None
                    }
                })
                .collect();
            info!("Replaying {} files", files.len());
            let chunks = files.chunks(max(files.len() / num_tasks, 1));
            let tasks = chunks.into_iter().map(|chunk| async move {
                for file in chunk {
                    info!("Replaying from state dump file {}", file);
                    let contents = std::fs::read_to_string(file).unwrap();
                    let sandbox_state: ExecutionSandboxState =
                        serde_json::from_str(&contents).unwrap();
                    let sandbox_state =
                        LocalExec::certificate_execute_with_sandbox_state(&sandbox_state)
                            .await
                            .unwrap();
                    sandbox_state.check_effects().unwrap();
                }
            });
            futures::future::join_all(tasks).await;

            // TODO: WTF is this
            Some((0u64, 0u64))
        }
        ReplayToolCommand::ProfileTransaction {
            tx_digest,
            executor_version,
            protocol_version,
            profile_output,
            config_objects,
        } => {
            let output_path = profile_output.or(Some(get_default_output_filepath()));

            let tx_digest = TransactionDigest::from_str(&tx_digest)?;
            info!("Executing tx: {}", tx_digest);
            let _sandbox_state = LocalExec::replay_with_network_config(
                get_rpc_url(rpc_url, cfg_path, chain)?,
                tx_digest,
                safety,
                use_authority,
                executor_version,
                protocol_version,
                output_path,
                parse_configs_versions(config_objects),
            )
            .await?;

            println!("Execution finished successfully.");
            Some((1u64, 1u64))
        }

        ReplayToolCommand::ReplayTransaction {
            tx_digest,
            show_effects,
            executor_version,
            protocol_version,
            config_objects,
        } => {
            let tx_digest = TransactionDigest::from_str(&tx_digest)?;
            info!("Executing tx: {}", tx_digest);
            let sandbox_state = LocalExec::replay_with_network_config(
                get_rpc_url(rpc_url, cfg_path, chain)?,
                tx_digest,
                safety,
                use_authority,
                executor_version,
                protocol_version,
                None,
                parse_configs_versions(config_objects),
            )
            .await?;

            if show_effects {
                println!("{}", sandbox_state.local_exec_effects);
            }

            sandbox_state.check_effects()?;

            println!("Execution finished successfully. Local and on-chain effects match.");
            Some((1u64, 1u64))
        }

        ReplayToolCommand::Report => {
            let mut lx =
                LocalExec::new_from_fn_url(&rpc_url.expect("Url must be provided")).await?;
            let epoch_table = lx.protocol_ver_to_epoch_map().await?;

            // We need this for other activities in this session
            lx.current_protocol_version = *epoch_table.keys().peekable().last().unwrap();

            println!(
                "  Protocol Version  |                Epoch Change TX               |      Epoch Range     |   Checkpoint Range   "
            );
            println!(
                "---------------------------------------------------------------------------------------------------------------"
            );

            for (
                protocol_version,
                ProtocolVersionSummary {
                    epoch_change_tx: tx_digest,
                    epoch_start: start_epoch,
                    epoch_end: end_epoch,
                    checkpoint_start,
                    checkpoint_end,
                    ..
                },
            ) in epoch_table
            {
                println!(
                    " {:^16}   | {:^43} | {:^10}-{:^10}| {:^10}-{:^10} ",
                    protocol_version,
                    tx_digest,
                    start_epoch,
                    end_epoch,
                    checkpoint_start.unwrap_or(u64::MAX),
                    checkpoint_end.unwrap_or(u64::MAX)
                );
            }

            lx.populate_protocol_version_tables().await?;
            for x in lx.protocol_version_system_package_table {
                println!("Protocol version: {}", x.0);
                for (package_id, seq_num) in x.1 {
                    println!("Package: {} Seq: {}", package_id, seq_num);
                }
            }
            None
        }

        ReplayToolCommand::ReplayCheckpoints {
            start,
            end,
            terminate_early,
            max_tasks,
        } => {
            assert!(start <= end, "Start checkpoint must be <= end checkpoint");
            assert!(max_tasks > 0, "Max tasks must be > 0");
            let checkpoints_per_task = ((end - start + max_tasks) / max_tasks) as usize;
            let mut handles = vec![];
            info!(
                "Executing checkpoints {} to {} with at most {} tasks and at most {} checkpoints per task",
                start, end, max_tasks, checkpoints_per_task
            );

            let range: Vec<_> = (start..=end).collect();
            for (task_count, checkpoints) in range.chunks(checkpoints_per_task).enumerate() {
                let checkpoints = checkpoints.to_vec();
                let rpc_url = rpc_url.clone();
                let safety = safety.clone();
                handles.push(tokio::spawn(async move {
                    info!("Spawning task {task_count} for checkpoints {checkpoints:?}");
                    let time = std::time::Instant::now();
                    let (succeeded, total) = LocalExec::new_from_fn_url(&rpc_url.expect("Url must be provided"))
                        .await
                        .unwrap()
                        .init_for_execution()
                        .await
                        .unwrap()
                        .execute_all_in_checkpoints(&checkpoints, &safety, terminate_early, use_authority)
                        .await
                        .unwrap();
                    let time = time.elapsed();
                    info!(
                        "Task {task_count}: executed checkpoints {:?} @ {} total transactions, {} succeeded",
                        checkpoints, total, succeeded
                    );
                    (succeeded, total, time)
                }));
            }

            let mut total_tx = 0;
            let mut total_time_ms = 0;
            let mut total_succeeded = 0;
            futures::future::join_all(handles)
                .await
                .into_iter()
                .for_each(|x| match x {
                    Ok((succeeded, total, time)) => {
                        total_tx += total;
                        total_time_ms += time.as_millis() as u64;
                        total_succeeded += succeeded;
                    }
                    Err(e) => {
                        error!("Task failed: {:?}", e);
                    }
                });
            info!(
                "Executed {} checkpoints @ {}/{} total TXs succeeded in {} ms ({}) avg TX/s",
                end - start + 1,
                total_succeeded,
                total_tx,
                total_time_ms,
                (total_tx as f64) / (total_time_ms as f64 / 1000.0)
            );
            Some((total_succeeded, total_tx))
        }
        ReplayToolCommand::ReplayEpoch {
            epoch,
            terminate_early,
            max_tasks,
        } => {
            let lx =
                LocalExec::new_from_fn_url(&rpc_url.clone().expect("Url must be provided")).await?;

            let (start, end) = lx.checkpoints_for_epoch(epoch).await?;

            info!(
                "Executing epoch {} (checkpoint range {}-{}) with at most {} tasks",
                epoch, start, end, max_tasks
            );
            let status = execute_replay_command(
                rpc_url,
                safety_checks,
                use_authority,
                cfg_path,
                chain,
                ReplayToolCommand::ReplayCheckpoints {
                    start,
                    end,
                    terminate_early,
                    max_tasks,
                },
            )
            .await;
            match status {
                Ok(Some((succeeded, total))) => {
                    info!(
                        "Epoch {} replay finished {} out of {} TXs",
                        epoch, succeeded, total
                    );

                    return Ok(Some((succeeded, total)));
                }
                Ok(None) => {
                    return Ok(None);
                }
                Err(e) => {
                    error!("Epoch {} replay failed: {:?}", epoch, e);
                    return Err(e);
                }
            }
        }
    })
}

pub(crate) fn chain_from_chain_id(chain: &str) -> Chain {
    let mainnet_chain_id = format!("{}", get_mainnet_chain_identifier());
    // TODO: Since testnet periodically resets, we need to ensure that the chain id
    // is updated to the latest one.
    let testnet_chain_id = format!("{}", get_testnet_chain_identifier());

    if mainnet_chain_id == chain {
        Chain::Mainnet
    } else if testnet_chain_id == chain {
        Chain::Testnet
    } else {
        Chain::Unknown
    }
}

fn parse_configs_versions(
    configs_and_versions: Option<Vec<String>>,
) -> Option<Vec<(ObjectID, SequenceNumber)>> {
    let configs_and_versions = configs_and_versions?;

    assert!(
        configs_and_versions.len() % 2 == 0,
        "Invalid number of arguments for configs and version -- you must supply a version for each config"
    );
    Some(
        configs_and_versions
            .chunks_exact(2)
            .map(|chunk| {
                let object_id =
                    ObjectID::from_str(&chunk[0]).expect("Invalid object id for config");
                let object_version = SequenceNumber::from_u64(
                    chunk[1]
                        .parse::<u64>()
                        .expect("Invalid object version for config"),
                );
                (object_id, object_version)
            })
            .collect(),
    )
}
