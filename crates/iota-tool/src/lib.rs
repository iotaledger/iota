// Copyright (c) 2021, Facebook, Inc. and its affiliates
// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::BTreeMap,
    env,
    fmt::Write,
    fs, io,
    num::NonZeroUsize,
    ops::Range,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
    time::Duration,
};

use anyhow::{Result, anyhow, bail};
use clap::ValueEnum;
use fastcrypto::{hash::MultisetHash, traits::ToFromBytes};
use futures::{
    StreamExt, TryStreamExt,
    future::{AbortHandle, join_all},
};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use iota_archival::{
    reader::{ArchiveReader, ArchiveReaderMetrics},
    verify_archive_with_checksums, verify_archive_with_genesis_config,
};
use iota_config::{
    NodeConfig,
    genesis::Genesis,
    node::ArchiveReaderConfig,
    object_storage_config::{ObjectStoreConfig, ObjectStoreType},
};
use iota_core::{
    authority::{AuthorityStore, authority_store_tables::AuthorityPerpetualTables},
    authority_client::{NetworkAuthorityClient, validator::ValidatorAPI},
    checkpoints::CheckpointStore,
    epoch::committee_store::CommitteeStore,
    execution_cache::build_execution_cache_from_env,
    grpc_indexes::{GRPC_INDEXES_DIR, GrpcIndexesStore},
    storage::RocksDbStore,
};
use iota_network::default_iota_network_config;
use iota_protocol_config::Chain;
use iota_sdk::{IotaClient, IotaClientBuilder};
use iota_sdk_types::{ObjectId, Owner};
use iota_snapshot::{
    VerifiedEpochInfo, reader::StateSnapshotReaderV1, restore::RestoreWithGrpcIndexes,
    setup_db_state,
};
use iota_storage::object_store::{
    ObjectStoreGetExt,
    http::HttpDownloaderBuilder,
    util::{MANIFEST_FILENAME, PerEpochManifest, RootManifest, copy_file, exists, get_path},
};
use iota_types::{
    base_types::*,
    committee::QUORUM_THRESHOLD,
    crypto::AuthorityPublicKeyBytes,
    digests::ChainIdentifier,
    global_state_hash::GlobalStateHash,
    messages_checkpoint::{CheckpointCommitment, ECMHLiveObjectSetDigest, VerifiedCheckpoint},
    messages_grpc::{
        LayoutGenerationOption, ObjectInfoRequest, ObjectInfoRequestKind, ObjectInfoResponse,
        TransactionInfoRequest, TransactionStatus,
    },
    multiaddr::Multiaddr,
    object::MoveObjectExt,
    storage::{ReadStore, SharedInMemoryStore},
};
use itertools::Itertools;
use prometheus::Registry;
use serde::{Deserialize, Serialize};
use tokio::{sync::mpsc, time::Instant};
use tracing::info;

pub mod commands;
pub mod db_tool;
pub mod fire_drill;
pub mod genesis_ceremony;
pub mod genesis_inspector;

#[derive(
    Clone, Serialize, Deserialize, Debug, PartialEq, Copy, PartialOrd, Ord, Eq, ValueEnum, Default,
)]
pub enum SnapshotVerifyMode {
    /// verification of both db state and downloaded checkpoints are skipped.
    /// This is the fastest mode, but is unsafe, and thus should only be used
    /// if you fully trust the source for both the snapshot and the checkpoint
    /// archive.
    None,
    /// verify snapshot state during download, but no post-restore db
    /// verification. Checkpoint verification is performed.
    #[default]
    Normal,
    /// In ADDITION to the behavior of `--verify normal`, verify db state
    /// post-restore against the end of epoch state root commitment.
    Strict,
}

// Make clients for fetching relevant data (objects, transactions, checkpoints)
// from the current committee members.
async fn make_clients(
    iota_client: &Arc<IotaClient>,
) -> Result<BTreeMap<AuthorityName, (Multiaddr, NetworkAuthorityClient)>> {
    let mut net_config = default_iota_network_config();
    net_config.connect_timeout = Some(Duration::from_secs(5));
    let mut authority_clients = BTreeMap::new();

    let state = iota_client
        .governance_api()
        .get_latest_iota_system_state()
        .await?;

    for committee_member in state.iter_committee_members() {
        let net_addr = Multiaddr::try_from(committee_member.net_address.clone())
            .unwrap()
            .rewrite_http_to_https();
        let tls_config = iota_tls::create_rustls_client_config(
            iota_types::crypto::NetworkPublicKey::from_bytes(
                &committee_member.network_pubkey_bytes,
            )?,
            iota_tls::IOTA_VALIDATOR_SERVER_NAME.to_string(),
            None,
        );
        let channel = net_config
            .connect_lazy(&net_addr, tls_config)
            .map_err(|err| anyhow!(err.to_string()))?;
        let client = NetworkAuthorityClient::new(channel);
        let public_key_bytes =
            AuthorityPublicKeyBytes::from_bytes(&committee_member.authority_pubkey_bytes)?;
        authority_clients.insert(public_key_bytes, (net_addr.clone(), client));
    }

    Ok(authority_clients)
}

type ObjectVersionResponses = (Option<SequenceNumber>, Result<ObjectInfoResponse>, f64);
pub struct ObjectData {
    requested_id: ObjectId,
    responses: Vec<(AuthorityName, Multiaddr, ObjectVersionResponses)>,
}

trait OptionDebug<T> {
    fn opt_debug(&self, def_str: &str) -> String;
}

impl<T> OptionDebug<T> for Option<T>
where
    T: std::fmt::Debug,
{
    fn opt_debug(&self, def_str: &str) -> String {
        match self {
            None => def_str.to_string(),
            Some(t) => format!("{t:?}"),
        }
    }
}

#[expect(clippy::type_complexity)]
pub struct GroupedObjectOutput {
    pub grouped_results: BTreeMap<
        Option<(
            Option<SequenceNumber>,
            ObjectDigest,
            TransactionDigest,
            Owner,
            Option<TransactionDigest>,
        )>,
        Vec<AuthorityName>,
    >,
    pub voting_power: Vec<(
        Option<(
            Option<SequenceNumber>,
            ObjectDigest,
            TransactionDigest,
            Owner,
            Option<TransactionDigest>,
        )>,
        u64,
    )>,
    pub available_voting_power: u64,
    pub fully_locked: bool,
}

impl GroupedObjectOutput {
    pub fn new(
        object_data: ObjectData,
        committee: Arc<BTreeMap<AuthorityPublicKeyBytes, u64>>,
    ) -> Self {
        let mut grouped_results = BTreeMap::new();
        let mut voting_power = BTreeMap::new();
        let mut available_voting_power = 0;
        for (name, _, (version, resp, _elapsed)) in &object_data.responses {
            let stake = committee.get(name).unwrap();
            let key = match resp {
                Ok(r) => {
                    let obj_digest = r.object.object_ref().digest;
                    let parent_tx_digest = r.object.previous_transaction;
                    let owner = r.object.owner;
                    let lock = r.lock_for_debugging.as_ref().map(|lock| *lock.digest());
                    if lock.is_none() {
                        available_voting_power += stake;
                    }
                    Some((*version, obj_digest, parent_tx_digest, owner, lock))
                }
                Err(_) => None,
            };
            let entry = grouped_results.entry(key).or_insert_with(Vec::new);
            entry.push(*name);
            let entry: &mut u64 = voting_power.entry(key).or_default();
            *entry += stake;
        }
        let voting_power = voting_power
            .into_iter()
            .sorted_by(|(_, v1), (_, v2)| Ord::cmp(v2, v1))
            .collect::<Vec<_>>();
        let mut fully_locked = false;
        if !voting_power.is_empty()
            && voting_power.first().unwrap().1 + available_voting_power < QUORUM_THRESHOLD
        {
            fully_locked = true;
        }
        Self {
            grouped_results,
            voting_power,
            available_voting_power,
            fully_locked,
        }
    }
}

impl std::fmt::Display for GroupedObjectOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "available stake: {}", self.available_voting_power)?;
        writeln!(f, "fully locked: {}", self.fully_locked)?;
        writeln!(f, "{:<100}\n", "-".repeat(100))?;
        for (key, stake) in &self.voting_power {
            let val = self.grouped_results.get(key).unwrap();
            writeln!(f, "total stake: {stake}")?;
            match key {
                Some((_version, obj_digest, parent_tx_digest, owner, lock)) => {
                    let lock = lock.opt_debug("no-known-lock");
                    writeln!(f, "obj ref: {obj_digest}")?;
                    writeln!(f, "parent tx: {parent_tx_digest}")?;
                    writeln!(f, "owner: {owner}")?;
                    writeln!(f, "lock: {lock}")?;
                    for (i, name) in val.iter().enumerate() {
                        writeln!(f, "        {:<4} {:<20}", i, name.concise(),)?;
                    }
                }
                None => {
                    writeln!(f, "ERROR")?;
                    for (i, name) in val.iter().enumerate() {
                        writeln!(f, "        {:<4} {:<20}", i, name.concise(),)?;
                    }
                }
            };
            writeln!(f, "{:<100}\n", "-".repeat(100))?;
        }
        Ok(())
    }
}

struct ConciseObjectOutput(ObjectData);

impl ConciseObjectOutput {
    fn header() -> String {
        format!(
            "{:<20} {:<8} {:<66} {:<45} {}",
            "validator", "version", "digest", "parent_cert", "owner"
        )
    }
}

impl std::fmt::Display for ConciseObjectOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (name, _multi_addr, (version, resp, _time_elapsed)) in &self.0.responses {
            write!(
                f,
                "{:<20} {:<8}",
                format!("{:?}", name.concise()),
                version.opt_debug("-")
            )?;
            match resp {
                Err(_) => writeln!(
                    f,
                    "{:<66} {:<45} {:<51}",
                    "object-fetch-failed", "no-cert-available", "no-owner-available"
                )?,
                Ok(resp) => {
                    let obj_digest = resp.object.object_ref().digest;
                    let parent = resp.object.previous_transaction;
                    let owner = resp.object.owner;
                    write!(f, " {obj_digest:<66} {parent:<45} {owner:<51}")?;
                }
            }
            writeln!(f)?;
        }
        Ok(())
    }
}

struct VerboseObjectOutput(ObjectData);

impl std::fmt::Display for VerboseObjectOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Object: {}", self.0.requested_id)?;

        for (name, multiaddr, (version, resp, timespent)) in &self.0.responses {
            writeln!(f, "validator: {:?}, addr: {:?}", name.concise(), multiaddr)?;
            writeln!(
                f,
                "-- version: {} ({:.3}s)",
                version.opt_debug("<version not available>"),
                timespent,
            )?;

            match resp {
                Err(e) => writeln!(f, "Error fetching object: {e}")?,
                Ok(resp) => {
                    writeln!(f, "  -- object digest: {}", resp.object.object_ref().digest)?;
                    if resp.object.is_package() {
                        writeln!(f, "  -- object: <Move Package>")?;
                    } else if let Some(layout) = &resp.layout {
                        writeln!(
                            f,
                            "  -- object: Move Object: {}",
                            resp.object
                                .data
                                .as_struct_opt()
                                .unwrap()
                                .to_move_struct(layout)
                                .unwrap()
                        )?;
                    }
                    writeln!(f, "  -- owner: {}", resp.object.owner)?;
                    writeln!(
                        f,
                        "  -- locked by: {}",
                        resp.lock_for_debugging.opt_debug("<not locked>")
                    )?;
                }
            }
        }
        Ok(())
    }
}

pub async fn get_object(
    obj_id: ObjectId,
    version: Option<u64>,
    validator: Option<AuthorityName>,
    clients: Arc<BTreeMap<AuthorityName, (Multiaddr, NetworkAuthorityClient)>>,
) -> Result<ObjectData> {
    let responses = join_all(
        clients
            .iter()
            .filter(|(name, _)| {
                if let Some(v) = validator {
                    v == **name
                } else {
                    true
                }
            })
            .map(|(name, (address, client))| async {
                let object_version = get_object_impl(client, obj_id, version).await;
                (*name, address.clone(), object_version)
            }),
    )
    .await;

    Ok(ObjectData {
        requested_id: obj_id,
        responses,
    })
}

pub async fn get_transaction_block(
    tx_digest: TransactionDigest,
    show_input_tx: bool,
    fullnode_rpc: String,
) -> Result<String> {
    let iota_client = Arc::new(IotaClientBuilder::default().build(fullnode_rpc).await?);
    let clients = make_clients(&iota_client).await?;
    let timer = Instant::now();
    let responses = join_all(clients.iter().map(|(name, (address, client))| async {
        let result = client
            .handle_transaction_info_request(TransactionInfoRequest {
                transaction_digest: tx_digest,
            })
            .await;
        (
            *name,
            address.clone(),
            result,
            timer.elapsed().as_secs_f64(),
        )
    }))
    .await;

    // Grab one validator that return Some(TransactionInfoResponse)
    let validator_aware_of_tx = responses.iter().find(|r| r.2.is_ok());

    let responses = responses
        .iter()
        .map(|r| {
            let key =
                r.2.as_ref()
                    .map(|ok_result| match &ok_result.status {
                        TransactionStatus::Signed(_) => None,
                        TransactionStatus::Executed(_, effects, _) => Some(effects.digest()),
                    })
                    .ok();
            let err = r.2.as_ref().err();
            (key, err, r)
        })
        .sorted_by(|(k1, err1, _), (k2, err2, _)| {
            Ord::cmp(k1, k2).then_with(|| Ord::cmp(err1, err2))
        })
        .chunk_by(|(_, _err, r)| {
            r.2.as_ref().map(|ok_result| match &ok_result.status {
                TransactionStatus::Signed(_) => None,
                TransactionStatus::Executed(_, effects, _) => Some((
                    ok_result.transaction.transaction_data(),
                    effects.data(),
                    effects.digest(),
                )),
            })
        });
    let mut s = String::new();
    for (i, (key, group)) in responses.into_iter().enumerate() {
        match key {
            Ok(Some((tx, effects, effects_digest))) => {
                writeln!(
                    &mut s,
                    "#{i:<2} tx_digest: {tx_digest:<68} effects_digest: {effects_digest}",
                )?;
                writeln!(&mut s, "{effects:#?}")?;
                if show_input_tx {
                    writeln!(&mut s, "{tx:#?}")?;
                }
            }
            Ok(None) => {
                writeln!(
                    &mut s,
                    "#{i:<2} tx_digest: {tx_digest:<68?} Signed but not executed"
                )?;
                if show_input_tx {
                    // In this case, we expect at least one validator knows about this tx
                    let validator_aware_of_tx = validator_aware_of_tx.unwrap();
                    let client = &clients.get(&validator_aware_of_tx.0).unwrap().1;
                    let tx_info = client.handle_transaction_info_request(TransactionInfoRequest {
                        transaction_digest: tx_digest,
                    }).await.unwrap_or_else(|e| panic!("Validator {:?} should have known about tx_digest: {:?}, got error: {:?}", validator_aware_of_tx.0, tx_digest, e));
                    writeln!(&mut s, "{tx_info:#?}")?;
                }
            }
            other => {
                writeln!(&mut s, "#{i:<2} {other:#?}")?;
            }
        }
        for (j, (_, _, res)) in group.enumerate() {
            writeln!(
                &mut s,
                "        {:<4} {:<20} {:<56} ({:.3}s)",
                j,
                res.0.concise(),
                format!("{}", res.1),
                res.3
            )?;
        }
        writeln!(&mut s, "{:<100}\n", "-".repeat(100))?;
    }
    Ok(s)
}

async fn get_object_impl(
    client: &NetworkAuthorityClient,
    id: ObjectId,
    version: Option<u64>,
) -> (Option<SequenceNumber>, Result<ObjectInfoResponse>, f64) {
    let start = Instant::now();
    let resp = client
        .handle_object_info_request(ObjectInfoRequest {
            object_id: id,
            generate_layout: LayoutGenerationOption::Generate,
            request_kind: match version {
                None => ObjectInfoRequestKind::LatestObjectInfo,
                Some(v) => ObjectInfoRequestKind::PastObjectInfoDebug(SequenceNumber::from_u64(v)),
            },
        })
        .await
        .map_err(anyhow::Error::from);
    let elapsed = start.elapsed().as_secs_f64();

    let resp_version = resp.as_ref().ok().map(|r| r.object.version());
    (resp_version, resp, elapsed)
}

pub(crate) fn make_anemo_config() -> anemo_cli::Config {
    use iota_network::{discovery::*, state_sync::*};

    // TODO: implement `ServiceInfo` generation in anemo-build and use here.
    anemo_cli::Config::new()
        // IOTA discovery
        .add_service(
            "Discovery",
            anemo_cli::ServiceInfo::new().add_method(
                "GetKnownPeersV2",
                anemo_cli::ron_method!(DiscoveryClient, get_known_peers_v2, ()),
            ),
        )
        // IOTA state sync
        .add_service(
            "StateSync",
            anemo_cli::ServiceInfo::new()
                .add_method(
                    "PushCheckpointSummary",
                    anemo_cli::ron_method!(
                        StateSyncClient,
                        push_checkpoint_summary,
                        iota_types::messages_checkpoint::CertifiedCheckpointSummary
                    ),
                )
                .add_method(
                    "GetCheckpointSummary",
                    anemo_cli::ron_method!(
                        StateSyncClient,
                        get_checkpoint_summary,
                        GetCheckpointSummaryRequest
                    ),
                )
                .add_method(
                    "GetCheckpointContents",
                    anemo_cli::ron_method!(
                        StateSyncClient,
                        get_checkpoint_contents,
                        iota_types::messages_checkpoint::CheckpointContentsDigest
                    ),
                )
                .add_method(
                    "GetCheckpointAvailability",
                    anemo_cli::ron_method!(StateSyncClient, get_checkpoint_availability, ()),
                ),
        )
}

fn copy_dir_all(
    src: impl AsRef<Path>,
    dst: impl AsRef<Path>,
    skip: Vec<PathBuf>,
) -> io::Result<()> {
    fs::create_dir_all(&dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if skip.contains(&entry.path()) {
            continue;
        }
        if ty.is_dir() {
            copy_dir_all(
                entry.path(),
                dst.as_ref().join(entry.file_name()),
                skip.clone(),
            )?;
        } else {
            fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
        }
    }
    Ok(())
}

pub async fn restore_from_db_checkpoint(
    config: &NodeConfig,
    db_checkpoint_path: &Path,
) -> Result<(), anyhow::Error> {
    copy_dir_all(db_checkpoint_path, config.db_path(), vec![])?;
    Ok(())
}

/// Insert the genesis checkpoint if the store doesn't hold it yet.
fn insert_genesis_checkpoint(
    checkpoint_store: &CheckpointStore,
    genesis: &Genesis,
) -> Result<(), anyhow::Error> {
    if checkpoint_store
        .get_checkpoint_by_digest(genesis.checkpoint().digest())?
        .is_none()
    {
        checkpoint_store.insert_checkpoint_contents(genesis.checkpoint_contents().clone())?;
        checkpoint_store.insert_verified_checkpoint(&genesis.checkpoint())?;
        checkpoint_store.update_highest_synced_checkpoint(&genesis.checkpoint())?;
    }
    Ok(())
}

/// Set all four checkpoint watermarks to the restore checkpoint.
///
/// SAFETY: they must be set together so the executor starts from
/// `highest_executed + 1` and never tries to access checkpoint contents in
/// the restored (summary-only) range.
fn set_restore_watermarks(
    checkpoint_store: &CheckpointStore,
    checkpoint: &VerifiedCheckpoint,
) -> Result<(), anyhow::Error> {
    checkpoint_store.update_highest_verified_checkpoint(checkpoint)?;
    checkpoint_store.update_highest_synced_checkpoint(checkpoint)?;
    checkpoint_store.update_highest_executed_checkpoint(checkpoint)?;
    checkpoint_store.update_highest_pruned_checkpoint(checkpoint)?;
    Ok(())
}

/// Backfill **every** checkpoint summary up to the node's highest synced
/// checkpoint from the checkpoint archive, into an existing (stopped) node's
/// checkpoint store at `node_db_path`.
///
/// A node restored from a formal snapshot holds only the end-of-epoch
/// summaries; this fills in every intermediate one so the node holds the
/// complete header chain from genesis (e.g. to serve historical checkpoint
/// queries, or to be a full summary source for syncing peers). It only adds
/// historical summaries below the node's existing watermarks — it never moves
/// a watermark, so the node's synced/executed/pruned state is untouched.
pub async fn backfill_checkpoint_summaries(
    node_db_path: &Path,
    num_parallel_downloads: usize,
) -> Result<(), anyhow::Error> {
    let m = MultiProgress::new();

    // Open the stopped node's existing stores in place. The committee store
    // already holds the genesis committee (from restore/sync), so it is opened
    // without re-supplying one.
    let perpetual_db = Arc::new(AuthorityPerpetualTables::open(
        &node_db_path.join("store"),
        None,
    ));
    let committee_store = Arc::new(CommitteeStore::open(node_db_path.join("epochs"), None)?);
    let checkpoint_store = CheckpointStore::new(&node_db_path.join("checkpoints"));
    let store = AuthorityStore::open_no_genesis(perpetual_db, false, &Registry::default())?;
    let cache_traits = build_execution_cache_from_env(&Registry::default(), &store);
    let state_sync_store =
        RocksDbStore::new(cache_traits, committee_store, checkpoint_store.clone());

    let highest_synced = checkpoint_store
        .get_highest_synced_checkpoint()?
        .map(|c| c.sequence_number)
        .ok_or_else(|| {
            anyhow!("checkpoint store at {node_db_path:?} is empty; restore the node first")
        })?;

    // Derive the network — and hence the archive bucket — from the node's own
    // genesis checkpoint.
    let genesis_checkpoint = checkpoint_store
        .get_checkpoint_by_sequence_number(0)?
        .ok_or_else(|| anyhow!("node has no genesis checkpoint; restore the node first"))?;
    let network = ChainIdentifier::from(*genesis_checkpoint.digest()).chain();

    let config = ArchiveReaderConfig {
        remote_store_config: default_archive_store_config(network),
        download_concurrency: NonZeroUsize::new(num_parallel_downloads).unwrap(),
        use_for_pruning_watermark: false,
    };
    let metrics = ArchiveReaderMetrics::new(&Registry::default());
    let archive_reader = ArchiveReader::new(config, &metrics)?;
    archive_reader.sync_manifest_once().await?;

    // Fill the contiguous range `[1, target]`; genesis (0) is the chain root
    // and is already present. Cap `target` at the archive's latest: summaries
    // above the restore point the node already holds in full from p2p, and the
    // archive may not reach as far as the node has synced.
    let archive_latest = archive_reader
        .get_manifest()
        .await?
        .next_checkpoint_seq_num()
        .saturating_sub(1);
    let target = highest_synced.min(archive_latest);
    if target == 0 {
        m.println("Nothing to backfill: no archived summaries below the node's state.")?;
        return Ok(());
    }
    if archive_latest < highest_synced {
        m.println(format!(
            "Note: the archive only reaches checkpoint {archive_latest}, below this node's \
             highest synced checkpoint {highest_synced}. Summaries in \
             ({archive_latest}, {highest_synced}] are left as the node already has them; re-run \
             once the archive catches up to fill any remaining gaps."
        ))?;
    }

    // Download and chain-verify summaries for `[1, target]` in one ordered
    // pass. `read_summaries_for_range` leaves already-present summaries
    // untouched and never persists an unverified one, so it adds only the
    // missing historical summaries below the node's watermarks without moving
    // any of them.
    let bar = m.add(ProgressBar::new(target).with_style(
        ProgressStyle::with_template("[{elapsed_precise}] {wide_bar} {pos}/{len} ({msg})").unwrap(),
    ));
    let counter = Arc::new(AtomicU64::new(0));
    spawn_rate_ticker(bar.clone(), counter.clone(), "checkpoints per sec");
    archive_reader
        .read_summaries_for_range(state_sync_store, 1..target + 1, counter)
        .await?;
    bar.finish_with_message("Checkpoint summary backfill is complete");

    println!("Backfilled checkpoint summaries up to checkpoint {target}");
    Ok(())
}

/// Build the checkpoint-archive `ObjectStoreConfig` for `network`: the
/// permissionless public archive by default, or a custom bucket when the
/// `CUSTOM_ARCHIVE_BUCKET` env vars are set.
fn default_archive_store_config(network: Chain) -> ObjectStoreConfig {
    let archive_bucket = Some(
        env::var("FORMAL_SNAPSHOT_ARCHIVE_BUCKET").unwrap_or_else(|_| match network {
            Chain::Mainnet => "iota-mainnet-archive".to_string(),
            Chain::Testnet => "iota-testnet-archive".to_string(),
            Chain::Unknown => {
                panic!("Cannot generate default archive bucket for unknown network");
            }
        }),
    );

    let custom_archive_enabled = env::var("CUSTOM_ARCHIVE_BUCKET").is_ok_and(|v| v == "true");
    if custom_archive_enabled {
        let aws_region =
            Some(env::var("FORMAL_SNAPSHOT_ARCHIVE_REGION").unwrap_or("us-west-2".to_string()));
        let archive_bucket_type = env::var("FORMAL_SNAPSHOT_ARCHIVE_BUCKET_TYPE").expect(
            "If setting `CUSTOM_ARCHIVE_BUCKET=true` Must set FORMAL_SNAPSHOT_ARCHIVE_BUCKET_TYPE, and credentials",
        );
        match archive_bucket_type.to_ascii_lowercase().as_str() {
            "s3" => ObjectStoreConfig {
                object_store: Some(ObjectStoreType::S3),
                bucket: archive_bucket.filter(|s| !s.is_empty()),
                aws_access_key_id: env::var("AWS_ARCHIVE_ACCESS_KEY_ID").ok(),
                aws_secret_access_key: env::var("AWS_ARCHIVE_SECRET_ACCESS_KEY").ok(),
                aws_region,
                aws_endpoint: env::var("AWS_ARCHIVE_ENDPOINT").ok(),
                aws_virtual_hosted_style_request: env::var("AWS_ARCHIVE_VIRTUAL_HOSTED_REQUESTS")
                    .ok()
                    .and_then(|b| b.parse().ok())
                    .unwrap_or(false),
                object_store_connection_limit: 50,
                no_sign_request: false,
                ..Default::default()
            },
            "gcs" => ObjectStoreConfig {
                object_store: Some(ObjectStoreType::GCS),
                bucket: archive_bucket,
                google_service_account: env::var("GCS_ARCHIVE_SERVICE_ACCOUNT_FILE_PATH").ok(),
                object_store_connection_limit: 50,
                no_sign_request: false,
                ..Default::default()
            },
            "azure" => ObjectStoreConfig {
                object_store: Some(ObjectStoreType::Azure),
                bucket: archive_bucket,
                azure_storage_account: env::var("AZURE_ARCHIVE_STORAGE_ACCOUNT").ok(),
                azure_storage_access_key: env::var("AZURE_ARCHIVE_STORAGE_ACCESS_KEY").ok(),
                object_store_connection_limit: 50,
                no_sign_request: false,
                ..Default::default()
            },
            _ => panic!(
                "If setting `CUSTOM_ARCHIVE_BUCKET=true` must set FORMAL_SNAPSHOT_ARCHIVE_BUCKET_TYPE to one of 'gcs', 'azure', or 's3' "
            ),
        }
    } else {
        // Default to the permissionless archive store.
        let aws_endpoint = env::var("AWS_ARCHIVE_ENDPOINT")
            .ok()
            .or_else(|| match network {
                Chain::Mainnet => Some("https://archive.mainnet.iota.cafe".to_string()),
                Chain::Testnet => Some("https://archive.testnet.iota.cafe".to_string()),
                Chain::Unknown => None,
            });
        let aws_virtual_hosted_style_request = env::var("AWS_ARCHIVE_VIRTUAL_HOSTED_REQUESTS")
            .ok()
            .and_then(|b| b.parse().ok())
            .unwrap_or(matches!(network, Chain::Mainnet | Chain::Testnet));
        ObjectStoreConfig {
            object_store: Some(ObjectStoreType::S3),
            bucket: archive_bucket.filter(|s| !s.is_empty()),
            aws_region: Some("us-west-2".to_string()),
            aws_endpoint,
            aws_virtual_hosted_style_request,
            object_store_connection_limit: 200,
            no_sign_request: true,
            ..Default::default()
        }
    }
}

/// Spawn a background task that, once per second until `bar` finishes, mirrors
/// `counter` into the bar's position and reports `{unit}: {rate}`.
fn spawn_rate_ticker(bar: ProgressBar, counter: Arc<AtomicU64>, unit: &'static str) {
    let start = Instant::now();
    tokio::spawn(async move {
        while !bar.is_finished() {
            let count = counter.load(Ordering::Relaxed);
            bar.set_position(count);
            bar.set_message(format!(
                "{unit}: {}",
                count as f64 / start.elapsed().as_secs_f64()
            ));
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });
}

/// Seed the checkpoint store from the snapshot's chain-verified EPOCH_INFO —
/// the archive-free default of the formal restore: the genesis checkpoint
/// plus every epoch's certified closing summary, with the committees handed
/// to the committee store.
fn sync_summaries_from_epoch_info(
    checkpoint_store: &CheckpointStore,
    committee_store: &CommitteeStore,
    genesis: &Genesis,
    verified: &VerifiedEpochInfo,
    epoch: EpochId,
) -> Result<(), anyhow::Error> {
    anyhow::ensure!(
        verified.entries().len() as u64 == epoch + 1,
        "EPOCH_INFO covers {} epochs but the restore targets the end of epoch {epoch}",
        verified.entries().len(),
    );

    insert_genesis_checkpoint(checkpoint_store, genesis)?;

    // `committees()[i + 1]` is the committee that entry `i`'s
    // `end_of_epoch_data` handed forward.
    for (entry, next_committee) in verified.entries().iter().zip(&verified.committees()[1..]) {
        committee_store.insert_new_committee(next_committee)?;
        checkpoint_store.insert_verified_checkpoint(&VerifiedCheckpoint::new_unchecked(
            entry.last_checkpoint_summary.clone(),
        ))?;
    }

    let last_checkpoint = VerifiedCheckpoint::new_unchecked(
        verified
            .entries()
            .last()
            .expect("length checked above")
            .last_checkpoint_summary
            .clone(),
    );
    set_restore_watermarks(checkpoint_store, &last_checkpoint)?;
    Ok(())
}

pub async fn get_latest_available_epoch(
    snapshot_store_config: &ObjectStoreConfig,
) -> Result<u64, anyhow::Error> {
    // Thin wrapper over the snapshot library; keeps the existing CLI API.
    iota_snapshot::reader::latest_available_epoch(snapshot_store_config).await
}

pub async fn check_completed_snapshot(
    snapshot_store_config: &ObjectStoreConfig,
    epoch: EpochId,
) -> Result<(), anyhow::Error> {
    let success_marker = format!("epoch_{epoch}/_SUCCESS");
    let remote_object_store = if snapshot_store_config.no_sign_request {
        snapshot_store_config.make_http()?
    } else {
        snapshot_store_config.make().map(Arc::new)?
    };
    if exists(&remote_object_store, &get_path(success_marker.as_str())).await {
        Ok(())
    } else {
        bail!(
            "missing success marker at {}/{}",
            snapshot_store_config.bucket.as_ref().unwrap_or(
                &snapshot_store_config
                    .clone()
                    .aws_endpoint
                    .unwrap_or("unknown_bucket".to_string())
            ),
            success_marker
        )
    }
}

pub async fn download_formal_snapshot(
    path: &Path,
    epoch: EpochId,
    genesis: &Path,
    snapshot_store_config: ObjectStoreConfig,
    num_parallel_downloads: usize,
    verify: SnapshotVerifyMode,
    skip_grpc_indexes: bool,
) -> Result<(), anyhow::Error> {
    let m = MultiProgress::new();
    m.println(format!(
        "Beginning formal snapshot restore to end of epoch {epoch}, verification mode: {verify:?}",
    ))?;
    let path = path.join("staging").to_path_buf();
    if path.exists() {
        fs::remove_dir_all(path.clone())?;
    }
    let perpetual_db = Arc::new(AuthorityPerpetualTables::open(&path.join("store"), None));
    let genesis = Genesis::load(genesis).unwrap();
    let genesis_committee = genesis.committee()?;
    let expected_chain_id = ChainIdentifier::from(*genesis.checkpoint().digest());

    // Download and chain-verify the snapshot's EPOCH_INFO up front (one small
    // file): every entry's certified closing summary is checked against the
    // committee chain walked from the operator's genesis. It drives the
    // default (archive-free) summary sync and the gRPC epoch seeding, and
    // rejects a wrong-network or tampered snapshot before anything large is
    // downloaded.
    let (snapshot_chain_id, epoch_info) =
        StateSnapshotReaderV1::read_epoch_info_only(epoch, &snapshot_store_config).await?;
    let verified_epoch_info = iota_snapshot::verify_epoch_info_chain(
        epoch_info,
        genesis_committee.clone(),
        snapshot_chain_id,
        expected_chain_id,
    )?;

    let committee_store = Arc::new(CommitteeStore::new(
        path.join("epochs"),
        &genesis_committee,
        None,
    ));
    let checkpoint_store = CheckpointStore::new(&path.join("checkpoints"));

    // Seed the end-of-epoch summaries and committees straight from the
    // chain-verified EPOCH_INFO — the restore needs no checkpoint archive. A
    // node that additionally wants the full intermediate summary history can
    // run `iota-tool backfill-checkpoint-summaries` afterwards.
    sync_summaries_from_epoch_info(
        &checkpoint_store,
        &committee_store,
        &genesis,
        &verified_epoch_info,
        epoch,
    )?;

    // Unless `--skip-grpc-indexes` is passed, the gRPC index store is built
    // from the same object stream that restores the perpetual tables, so a
    // fullnode started with gRPC enabled opens it in place instead of
    // re-indexing the whole restored state.
    //
    // Like every other store of this restore, it lives under `staging/`,
    // which replaces `live/` wholesale at the end — so a pre-existing gRPC
    // index store (whatever its watermarks claim) can never survive into the
    // restored node and compete with the one built here.
    let grpc_indexes = (!skip_grpc_indexes).then(|| {
        Arc::new(GrpcIndexesStore::new_without_init(
            path.join(GRPC_INDEXES_DIR),
        ))
    });

    let (_abort_handle, abort_registration) = AbortHandle::new_pair();
    let perpetual_db_clone = perpetual_db.clone();
    let snapshot_dir = path.parent().unwrap().join("snapshot");
    if snapshot_dir.exists() {
        fs::remove_dir_all(snapshot_dir.clone())?;
    }
    let snapshot_dir_clone = snapshot_dir.clone();

    // TODO if verify is false, we should skip generating these and
    // not pass in a channel to the reader
    let (sender, mut receiver) = mpsc::channel(num_parallel_downloads);
    let m_clone = m.clone();
    let grpc_indexes_clone = grpc_indexes.clone();

    let snapshot_handle = tokio::spawn(async move {
        let local_store_config = ObjectStoreConfig {
            object_store: Some(ObjectStoreType::File),
            directory: Some(snapshot_dir_clone.to_path_buf()),
            ..Default::default()
        };
        let mut reader = StateSnapshotReaderV1::new(
            epoch,
            &snapshot_store_config,
            &local_store_config,
            NonZeroUsize::new(num_parallel_downloads).unwrap(),
            m_clone,
            false, // skip_reset_local_store
        )
        .await
        .unwrap_or_else(|err| panic!("Failed to create reader: {err}"));
        if let Some(grpc_indexes) = &grpc_indexes_clone {
            let grpc_restorer = grpc_indexes.live_object_restorer();
            let restore_target = RestoreWithGrpcIndexes::new(&perpetual_db_clone, &grpc_restorer);
            reader
                .read_to_db(&restore_target, abort_registration, Some(sender))
                .await
                .unwrap_or_else(|err| panic!("Failed during read: {err}"));
            grpc_restorer
                .finish()
                .unwrap_or_else(|err| panic!("Failed to flush the gRPC coin index: {err}"));
        } else {
            reader
                .read(&perpetual_db_clone, abort_registration, Some(sender))
                .await
                .unwrap_or_else(|err| panic!("Failed during read: {err}"));
        }

        Ok::<(), anyhow::Error>(())
    });
    let mut root_global_state_hash = GlobalStateHash::default();
    let mut num_live_objects = 0;
    while let Some((partial_hash, num_objects)) = receiver.recv().await {
        num_live_objects += num_objects;
        root_global_state_hash.union(&partial_hash);
    }

    let last_checkpoint = checkpoint_store
        .get_highest_verified_checkpoint()?
        .expect("Expected nonempty checkpoint store");

    // Perform snapshot state verification
    if verify != SnapshotVerifyMode::None {
        assert_eq!(
            last_checkpoint.epoch(),
            epoch,
            "Expected highest verified checkpoint ({}) to be for epoch {} but was for epoch {}",
            last_checkpoint.sequence_number,
            epoch,
            last_checkpoint.epoch()
        );
        let commitment = last_checkpoint
            .end_of_epoch_data
            .as_ref()
            .expect("Expected highest verified checkpoint to have end of epoch data")
            .epoch_commitments
            .last()
            .expect(
                "End of epoch has no commitments. This likely means that the epoch \
                you are attempting to restore from does not support end of epoch state \
                digest commitment. If restoring from mainnet, `--epoch` must be > 20, \
                and for testnet, `--epoch` must be > 12.",
            );
        match commitment {
            CheckpointCommitment::ECMHLiveObjectSetDigest(consensus_digest) => {
                let local_digest: ECMHLiveObjectSetDigest = root_global_state_hash.digest().into();
                assert_eq!(
                    *consensus_digest, local_digest,
                    "End of epoch {} root state digest {} does not match \
                    local root state hash {} computed from snapshot data",
                    epoch, consensus_digest.digest, local_digest.digest,
                );
                let progress_bar = m.add(
                    ProgressBar::new(1).with_style(
                        ProgressStyle::with_template(
                            "[{elapsed_precise}] {wide_bar} Verifying snapshot contents against root state hash ({msg})",
                        )
                        .unwrap(),
                    ),
                );
                progress_bar.finish_with_message("Verification complete");
            }
        };
    } else {
        m.println(
            "WARNING: Skipping snapshot verification! \
            This is highly discouraged unless you fully trust the source of this snapshot and its contents.
            If this was unintentional, rerun with `--verify` set to `normal` or `strict`.",
        )?;
    }

    snapshot_handle
        .await
        .expect("Task join failed")
        .expect("Snapshot restore task failed");

    setup_db_state(
        epoch,
        root_global_state_hash.clone(),
        perpetual_db.clone(),
        checkpoint_store.clone(),
        committee_store,
        verify == SnapshotVerifyMode::Strict,
        num_live_objects,
        m,
    )
    .await?;

    // Finish the gRPC index store (unless skipped): the live-state indexes
    // were built while the objects streamed in, so what's left is the epoch
    // rows from EPOCH_INFO, the open epoch's row, and the finalize that makes
    // the node open the store in place instead of re-indexing. All RocksDB
    // handles are dropped before the rename below.
    if let Some(grpc_indexes) = grpc_indexes {
        verified_epoch_info
            .restore_epoch_info(&*grpc_indexes)
            .await?;
        let authority_store =
            AuthorityStore::open_no_genesis(perpetual_db.clone(), false, &Registry::default())?;
        grpc_indexes.ensure_current_epoch_info(&authority_store, &checkpoint_store)?;
        grpc_indexes.finalize_restore(last_checkpoint.sequence_number)?;
        Arc::into_inner(grpc_indexes)
            .expect("the snapshot task is awaited, so its store handle is gone");
    }

    let new_path = path.parent().unwrap().join("live");
    if new_path.exists() {
        fs::remove_dir_all(new_path.clone())?;
    }
    fs::rename(&path, &new_path)?;
    fs::remove_dir_all(snapshot_dir.clone())?;
    println!("Successfully restored state from snapshot at end of epoch {epoch}");

    Ok(())
}

pub async fn download_db_snapshot(
    path: &Path,
    epoch: u64,
    snapshot_store_config: ObjectStoreConfig,
    skip_indexes: bool,
    num_parallel_downloads: usize,
) -> Result<(), anyhow::Error> {
    let remote_store = if snapshot_store_config.no_sign_request {
        snapshot_store_config.make_http()?
    } else {
        snapshot_store_config.make().map(Arc::new)?
    };

    // We rely on the top level MANIFEST file which contains all valid epochs
    let manifest_contents = remote_store.get_bytes(&get_path(MANIFEST_FILENAME)).await?;
    let root_manifest = RootManifest::from_bytes(&manifest_contents)
        .map_err(|err| anyhow!("Error parsing MANIFEST from bytes: {}", err))?;

    if !root_manifest.epoch_exists(epoch) {
        bail!("Epoch dir {} doesn't exist on the remote store", epoch);
    }

    let epoch_path = format!("epoch_{epoch}");
    let epoch_dir = get_path(&epoch_path);

    let manifest_file = epoch_dir.child(MANIFEST_FILENAME);
    let epoch_manifest_contents =
        String::from_utf8(remote_store.get_bytes(&manifest_file).await?.to_vec())
            .map_err(|err| anyhow!("Error parsing {}/MANIFEST from bytes: {}", epoch_path, err))?;

    let epoch_manifest =
        PerEpochManifest::deserialize_from_newline_delimited(&epoch_manifest_contents);

    let mut files: Vec<String> = vec![];
    files.extend(epoch_manifest.filter_by_prefix("store/perpetual").lines);
    files.extend(epoch_manifest.filter_by_prefix("epochs").lines);
    files.extend(epoch_manifest.filter_by_prefix("checkpoints").lines);
    if !skip_indexes {
        files.extend(epoch_manifest.filter_by_prefix("indexes").lines);
        files.extend(epoch_manifest.filter_by_prefix("grpc_indexes").lines);
    }
    let local_store = ObjectStoreConfig {
        object_store: Some(ObjectStoreType::File),
        directory: Some(path.to_path_buf()),
        ..Default::default()
    }
    .make()?;
    let m = MultiProgress::new();
    let path = path.to_path_buf();
    let snapshot_handle = tokio::spawn(async move {
        let progress_bar = m.add(
            ProgressBar::new(files.len() as u64).with_style(
                ProgressStyle::with_template(
                    "[{elapsed_precise}] {wide_bar} {pos} out of {len} files done ({msg})",
                )
                .unwrap(),
            ),
        );
        let cloned_progress_bar = progress_bar.clone();
        let file_counter = Arc::new(AtomicUsize::new(0));
        futures::stream::iter(files.iter())
            .map(|file| {
                let local_store = local_store.clone();
                let remote_store = remote_store.clone();
                let counter_cloned = file_counter.clone();
                async move {
                    counter_cloned.fetch_add(1, Ordering::Relaxed);
                    let file_path = get_path(format!("epoch_{epoch}/{file}").as_str());
                    copy_file(&file_path, &file_path, &remote_store, &local_store).await?;
                    Ok::<::object_store::path::Path, anyhow::Error>(file_path.clone())
                }
            })
            .boxed()
            .buffer_unordered(num_parallel_downloads)
            .try_for_each(|path| {
                file_counter.fetch_sub(1, Ordering::Relaxed);
                cloned_progress_bar.inc(1);
                cloned_progress_bar.set_message(format!(
                    "Downloading file: {}, #downloads_in_progress: {}",
                    path,
                    file_counter.load(Ordering::Relaxed)
                ));
                futures::future::ready(Ok(()))
            })
            .await?;
        progress_bar.finish_with_message("Snapshot file download is complete");
        Ok::<(), anyhow::Error>(())
    });

    let tasks: Vec<_> = vec![Box::pin(snapshot_handle)];
    join_all(tasks)
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .for_each(|result| result.expect("Task failed"));

    let store_dir = path.join("store");
    if store_dir.exists() {
        fs::remove_dir_all(&store_dir)?;
    }
    let epochs_dir = path.join("epochs");
    if epochs_dir.exists() {
        fs::remove_dir_all(&epochs_dir)?;
    }
    Ok(())
}

pub async fn verify_archive(
    genesis: &Path,
    remote_store_config: ObjectStoreConfig,
    concurrency: usize,
    interactive: bool,
) -> Result<()> {
    verify_archive_with_genesis_config(genesis, remote_store_config, concurrency, interactive, 10)
        .await
}

pub async fn dump_checkpoints_from_archive(
    remote_store_config: ObjectStoreConfig,
    start_checkpoint: u64,
    end_checkpoint: u64,
    max_content_length: usize,
) -> Result<()> {
    let metrics = ArchiveReaderMetrics::new(&Registry::default());
    let config = ArchiveReaderConfig {
        remote_store_config,
        download_concurrency: NonZeroUsize::new(1).unwrap(),
        use_for_pruning_watermark: false,
    };
    let store = SharedInMemoryStore::default();
    let archive_reader = ArchiveReader::new(config, &metrics)?;
    archive_reader.sync_manifest_once().await?;
    let checkpoint_counter = Arc::new(AtomicU64::new(0));
    let txn_counter = Arc::new(AtomicU64::new(0));
    archive_reader
        .read(
            store.clone(),
            Range {
                start: start_checkpoint,
                end: end_checkpoint,
            },
            txn_counter,
            checkpoint_counter,
            false,
        )
        .await?;
    for key in store
        .inner()
        .checkpoints()
        .values()
        .sorted_by(|a, b| a.sequence_number().cmp(&b.sequence_number))
    {
        let mut content = serde_json::to_string(
            &store
                .try_get_full_checkpoint_contents_by_sequence_number(key.sequence_number)?
                .unwrap(),
        )?;
        content.truncate(max_content_length);
        info!(
            "{}:{}:{:?}",
            key.sequence_number, key.content_digest, content
        );
    }
    Ok(())
}

pub async fn verify_archive_by_checksum(
    remote_store_config: ObjectStoreConfig,
    concurrency: usize,
) -> Result<()> {
    verify_archive_with_checksums(remote_store_config, concurrency).await
}
