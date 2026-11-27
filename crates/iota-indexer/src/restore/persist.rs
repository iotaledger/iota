// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::{BTreeMap, HashSet},
    time::Duration,
};

use bytes::Bytes;
use diesel::connection::SimpleConnection;
use downcast::Any;
use fastcrypto::hash::{HashFunction, Sha3_256};
use iota_core::authority::authority_store_tables::LiveObject as SnapshotObject;
use iota_sdk_types::{MoveStruct, ObjectId};
use iota_snapshot::{FileMetadata, VerifiedEpochInfo, reader::LiveObjectIter, restore::Restore};
use iota_storage::SHA3_BYTES;
use iota_types::{
    digests::ChainIdentifier, iota_system_state::IotaSystemStateTrait, object::Object,
};
use itertools::Itertools;
use strum::IntoEnumIterator;
use tokio::sync::Mutex;

use crate::{
    chunk,
    errors::{IndexerError, IndexerResult},
    ingestion::{
        common::{persist::CommitterTables, prepare::LiveObject},
        primary::persist::EpochToCommit,
    },
    models::{
        checkpoints::StoredChainIdentifier,
        display::StoredDisplay,
        epoch::{EndOfEpochUpdate, StartOfEpochUpdate, extract_epoch_info_event},
        obj_indices::StoredObjectVersion,
        objects::StoredCheckpointedObject,
        packages::StoredPackage,
        smart_account_keys::{StoredSmartAccountKey, public_key_field_tag, smart_account_tag},
    },
    pruning::pruner::PrunableTable,
    store::{IndexerStore, PgIndexerStore, diesel_macro::*},
    types::{IndexedCheckpoint, IndexedPackage},
};

/// Data derived from the live-object set included in the snapshot.
#[derive(Default)]
struct ObjectDerivedData {
    hasher: Sha3_256,
    displays: BTreeMap<String, StoredDisplay>,
    packages: Vec<StoredPackage>,
    object_versions: Vec<StoredObjectVersion>,
}

impl ObjectDerivedData {
    fn extend(&mut self, object: &Object, checkpoint_sequence_number: u64) {
        self.hasher.update(object.object_ref().digest.bytes());
        if let Some(display) = StoredDisplay::try_from_object(object) {
            self.displays.insert(display.object_type.clone(), display);
        }
        if let iota_sdk_types::ObjectData::Package(package) = object.data() {
            self.packages
                .push(IndexedPackage::new(package.clone(), checkpoint_sequence_number).into());
        }
        self.object_versions.push(StoredObjectVersion {
            object_id: object.id().as_bytes().to_vec(),
            object_version: object.version().as_u64() as i64,
            cp_sequence_number: checkpoint_sequence_number as i64,
        })
    }
}

/// Key rows and `SmartAccount` ids collected from the snapshot partitions.
/// A key and its account can be in different partitions, so the rows are
/// only filtered after the last partition is inserted.
#[derive(Default)]
struct SmartAccountKeys {
    /// One row for every public key field, whatever its parent is.
    keys: Vec<StoredSmartAccountKey>,
    /// The id of every `SmartAccount`, used to keep only the rows whose
    /// parent is one.
    smart_accounts: HashSet<ObjectId>,
}

impl SmartAccountKeys {
    /// Adds a row for every public key field in `objects` and the id of every
    /// `SmartAccount`.
    fn add_objects<'a>(
        &mut self,
        objects: impl IntoIterator<Item = &'a Object>,
    ) -> Result<(), IndexerError> {
        let key_field_tag = public_key_field_tag();
        let smart_account_tag = smart_account_tag();
        for object in objects {
            let Some(tag) = object.data.as_opt_struct().map(MoveStruct::struct_tag) else {
                continue;
            };
            if *tag == smart_account_tag {
                self.smart_accounts.insert(object.id());
            } else if *tag == key_field_tag {
                self.keys
                    .push(StoredSmartAccountKey::from_key_field(object)?);
            }
        }
        Ok(())
    }

    /// Moves the rows and ids of `other` into `self`.
    fn append(&mut self, other: Self) {
        self.keys.extend(other.keys);
        self.smart_accounts.extend(other.smart_accounts);
    }

    /// Returns the rows whose parent is a `SmartAccount`.
    fn into_rows(self) -> Vec<StoredSmartAccountKey> {
        let Self {
            keys,
            smart_accounts,
        } = self;
        keys.into_iter()
            .filter(|key| {
                ObjectId::from_bytes(&key.account_id).is_ok_and(|id| smart_accounts.contains(&id))
            })
            .collect()
    }
}

/// Inserts the snapshot partitions through `store` and collects the
/// `smart_account_keys` rows across them.
pub(crate) struct RestoreStore<'a> {
    store: &'a PgIndexerStore,
    smart_account_keys: Mutex<SmartAccountKeys>,
}

impl<'a> RestoreStore<'a> {
    pub(crate) fn new(store: &'a PgIndexerStore) -> Self {
        Self {
            store,
            smart_account_keys: Mutex::new(SmartAccountKeys::default()),
        }
    }

    /// Returns the rows of the key fields whose parent is a `SmartAccount`.
    /// Shall be called after every partition was inserted.
    pub(crate) fn into_smart_account_keys(self) -> Vec<StoredSmartAccountKey> {
        self.smart_account_keys.into_inner().into_rows()
    }
}

impl Restore for RestoreStore<'_> {
    async fn insert_partition(
        &self,
        file_metadata: FileMetadata,
        bytes: Bytes,
        expected_checksum: &[u8; SHA3_BYTES],
    ) -> anyhow::Result<()> {
        let partition_keys = self
            .store
            .insert_partition(file_metadata, bytes, expected_checksum)
            .await?;
        self.smart_account_keys.lock().await.append(partition_keys);
        Ok(())
    }
}

impl PgIndexerStore {
    /// Inserts one snapshot partition and returns its `smart_account_keys`
    /// rows and `SmartAccount` ids.
    async fn insert_partition(
        &self,
        file_metadata: FileMetadata,
        bytes: Bytes,
        expected_checksum: &[u8; SHA3_BYTES],
    ) -> anyhow::Result<SmartAccountKeys> {
        let mut derived_data = ObjectDerivedData::default();
        let partition = LiveObjectIter::new(&file_metadata, bytes)?.scan(
            &mut derived_data,
            |derived_data, snapshot_object| {
                let SnapshotObject {
                    object,
                    previous_transaction_checkpoint,
                } = snapshot_object;
                let checkpoint_sequence_number =
                    previous_transaction_checkpoint.unwrap_or_default();
                derived_data.extend(&object, checkpoint_sequence_number);
                Some(LiveObject::new(checkpoint_sequence_number, object))
            },
        );
        let chunks = chunk!(partition, self.config.parallel_objects_chunk_size);
        let sha3_digest = derived_data.hasher.finalize().digest;
        if *expected_checksum != sha3_digest {
            tracing::error!(
                "sha does not match! expected: {expected_checksum:?}, actual: {sha3_digest:?}",
            );
            anyhow::bail!(
                "checksum verification failed for bucket/partition: {}/{}",
                file_metadata.bucket_num,
                file_metadata.part_num
            );
        }
        let mut smart_account_keys = SmartAccountKeys::default();
        smart_account_keys.add_objects(chunks.iter().flatten().map(LiveObject::object))?;

        let (live_chunks, checkpointed_chunks): (Vec<_>, Vec<_>) = chunks
            .into_iter()
            .map(|c| {
                let checkpointed = c
                    .iter()
                    .map(|live| StoredCheckpointedObject::try_from(live.indexed_object.clone()))
                    .collect::<Result<Vec<_>, IndexerError>>()?;
                Ok::<_, IndexerError>((c, checkpointed))
            })
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .unzip();

        let live_tasks = live_chunks
            .into_iter()
            .map(|c| self.spawn_blocking_task(move |this| this.persist_live_objects(c)));
        futures::future::try_join_all(live_tasks)
            .await
            .map_err(|e| {
                tracing::error!(
                    "failed to join futures for persisting formal snapshot partition: {e}"
                );
                IndexerError::from(e)
            })?
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                tracing::error!("failed to persist all formal snapshot object chunks: {e:?}");
                IndexerError::PostgresWrite
            })?;

        let checkpointed_tasks = checkpointed_chunks.into_iter().map(|c| {
            self.spawn_blocking_task(move |this| this.persist_checkpointed_objects_chunk(c))
        });
        futures::future::try_join_all(checkpointed_tasks)
            .await
            .map_err(|e| {
                tracing::error!(
                    "failed to join futures for persisting formal snapshot partition to checkpointed_objects: {e}"
                );
                IndexerError::from(e)
            })?
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                tracing::error!("failed to persist all formal snapshot checkpointed object chunks: {e:?}");
                IndexerError::PostgresWrite
            })?;
        self.persist_displays(derived_data.displays.into_values().collect())
            .await?;
        self.persist_packages(derived_data.packages).await?;
        self.persist_object_versions(derived_data.object_versions)
            .await?;
        Ok(smart_account_keys)
    }
}

impl EpochToCommit {
    /// Builds the epoch commits for a verified snapshot.
    ///
    /// Returns one commit per epoch from 0 through `snapshot_epoch + 1`, in
    /// epoch order. Each entry closes its own epoch and opens the next, exactly
    /// as live ingestion does at an end-of-epoch checkpoint. The leading commit
    /// opens the genesis epoch.
    pub(crate) fn batch_from_verified_epoch_info(
        verified_epoch_info: VerifiedEpochInfo,
    ) -> Vec<Self> {
        let (epoch_info, _committees, start_system_states) = verified_epoch_info.into_parts();
        let epoch_info_entries = epoch_info.into_entries();

        // Initialize with the genesis epoch to commit.
        let genesis_system_state = start_system_states[0]
            .clone()
            .into_iota_system_state_summary();
        let mut epochs_to_commit = vec![EpochToCommit {
            last_epoch: None,
            new_epoch: StartOfEpochUpdate::new(&genesis_system_state, 0, 0, None),
        }];

        for (epoch, epoch_info_entry) in epoch_info_entries.iter().enumerate() {
            let event = extract_epoch_info_event(&epoch_info_entry.end_of_epoch_tx_events)
                .unwrap_or_default();
            let last_checkpoint_summary = &epoch_info_entry.last_checkpoint_summary;
            let new_epoch_system_state = start_system_states[epoch + 1]
                .clone()
                .into_iota_system_state_summary();
            let new_epoch_first_checkpoint_id = last_checkpoint_summary.sequence_number() + 1;
            let new_epoch_first_tx_sequence_number =
                last_checkpoint_summary.network_total_transactions;
            epochs_to_commit.push(EpochToCommit {
                last_epoch: Some(EndOfEpochUpdate::new(last_checkpoint_summary, &event)),
                new_epoch: StartOfEpochUpdate::new(
                    &new_epoch_system_state,
                    new_epoch_first_checkpoint_id,
                    new_epoch_first_tx_sequence_number,
                    Some(&event),
                ),
            });
        }
        epochs_to_commit
    }
}

async fn populate_chain_id(store: &PgIndexerStore, chain_id: ChainIdentifier) -> IndexerResult<()> {
    let checkpoint_digest = chain_id.digest().into_bytes().to_vec();
    store
        .execute_in_blocking_worker(|this| {
            this.persist_chain_identifier(StoredChainIdentifier { checkpoint_digest })
        })
        .await
}

async fn persist_smart_account_keys(
    store: &PgIndexerStore,
    keys: Vec<StoredSmartAccountKey>,
) -> IndexerResult<()> {
    store
        .execute_in_blocking_worker(move |this| this.persist_smart_account_keys_chunk(keys))
        .await
}

async fn populate_protocol_and_feature_flags(
    store: &PgIndexerStore,
    chain_id: ChainIdentifier,
) -> IndexerResult<()> {
    let checkpoint_digest = chain_id.digest().into_bytes().to_vec();
    store
        .execute_in_blocking_worker(move |this| {
            this.persist_protocol_configs_and_feature_flags(checkpoint_digest)
        })
        .await
}

async fn populate_epochs(
    store: &PgIndexerStore,
    verified_epoch_info: VerifiedEpochInfo,
) -> IndexerResult<()> {
    let epochs_to_commit = EpochToCommit::batch_from_verified_epoch_info(verified_epoch_info);
    store
        .execute_in_blocking_worker(move |this| this.persist_epochs(epochs_to_commit))
        .await
}

const EPOCH_PARTITIONED_TABLES: [&str; 2] = ["transactions", "events"];

/// Moves the initial partition of `transactions`, and `events` to `epoch`.
///
/// The migrations create a single `<table>_partition_0` that eludes the pruner,
/// and grows indefinitely.
///
/// Herein, that partition is dropped, and the partition of the restore epoch
/// is created instead with the respective lower bound.
///
/// # Errors
///
/// Returns an error if the database rejects the partition restore.
async fn restore_partitions(
    store: &PgIndexerStore,
    epoch: u64,
    epoch_start_tx: u64,
) -> IndexerResult<()> {
    store
        .execute_in_blocking_worker(move |this| {
            let pool = this.blocking_cp();
            transactional_blocking_with_retry!(
                &pool,
                |conn| {
                    let query = EPOCH_PARTITIONED_TABLES
                        .iter()
                        .map(|table| {
                            format!(
                                "DROP TABLE {table}_partition_0;
                                 CREATE TABLE {table}_partition_{epoch} PARTITION OF {table}
                                     FOR VALUES FROM ({epoch_start_tx}) TO (MAXVALUE);"
                            )
                        })
                        .join("\n");
                    conn.batch_execute(&query)?;
                    Ok::<(), IndexerError>(())
                },
                Duration::from_secs(10)
            )?;
            tracing::info!("Moved the initial epoch partitions to epoch {epoch}");
            Ok(())
        })
        .await
}

/// We populate the remaining tables after the objects.
///
/// This includes `epochs`, `chain_identifier` which we populate in parallel
/// with setting the checkpoint watermark to the last checkpoint of the snapshot
/// epoch.
///
/// We also move the initial partition of `transactions`, and `events` to the
/// epoch that follows the snapshot, and write the `smart_account_keys` rows
/// collected from the snapshot partitions.
///
/// Finally we populate `protocol_configs`, `feature_flags` up to the
/// protocol version that the snapshot epoch corresponds to, and the
/// `watermarks` table with the lower bounds associated with the epoch following
/// the snapshot.
pub(crate) async fn populate_remaining_tables(
    store: &PgIndexerStore,
    verified_epoch_info: VerifiedEpochInfo,
    snapshot_chain_id: ChainIdentifier,
    smart_account_keys: Vec<StoredSmartAccountKey>,
) -> IndexerResult<()> {
    let snapshot_epoch_boundary = verified_epoch_info.snapshot_entry();
    let sync_watermark = IndexedCheckpoint::from_iota_checkpoint(
        &snapshot_epoch_boundary.last_checkpoint_summary,
        &snapshot_epoch_boundary.last_checkpoint_contents,
        Default::default(), // We don't store this as part of the checkpoint so it's ok to set to 0
    );
    let next_epoch = sync_watermark.epoch + 1;
    let next_epoch_start_tx = sync_watermark.network_total_transactions;
    let pruning_watermarks: Vec<_> = PrunableTable::iter()
        .map(|table| (table, next_epoch))
        .collect();
    tokio::try_join!(
        populate_epochs(store, verified_epoch_info),
        populate_chain_id(store, snapshot_chain_id),
        store.persist_checkpoints(vec![sync_watermark]),
        restore_partitions(store, next_epoch, next_epoch_start_tx),
        persist_smart_account_keys(store, smart_account_keys),
    )?;
    tokio::try_join!(
        populate_protocol_and_feature_flags(store, snapshot_chain_id),
        store.update_watermarks_lower_bound(pruning_watermarks.clone()),
        // The restore effectively folds the object_versions table to the
        // set of objects that were live at the end of the target epoch.
        //
        // The next epoch is the one where the table starts getting filled with historical
        // versions again. Hence we use the respective lower bounds to represent that point in the
        // history of the table.
        store.update_watermarks_lower_bound(vec![(CommitterTables::ObjectsVersion, next_epoch)]),
    )?;
    // Finally align the lowest unpruned key with the lower bounds
    // to let the pruner know the pruning range start after restoring
    let (stored_watermarks, _) = store.get_watermarks().await?;
    let lowest_unpruned_keys = stored_watermarks
        .iter()
        .filter_map(|watermark| {
            let table = PrunableTable::try_from(watermark).ok()?;
            Some((table, table.pruning_strategy().range_end(watermark)))
        })
        .collect::<Vec<_>>();
    tokio::try_join!(
        store.update_watermarks_lowest_unpruned_key(lowest_unpruned_keys),
        // Same as for the lower bounds, we set the lowest_unpruned_key to
        // the next epoch.
        store.update_watermarks_lowest_unpruned_key(vec![(
            CommitterTables::ObjectsVersion,
            next_epoch
        )]),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use iota_sdk_move_types::iota_framework::{dynamic_field::Field, object::UID};
    use iota_sdk_types::{Address, Identifier, ObjectData, Owner, StructTag, TransactionDigest};
    use iota_types::{
        account_abstraction::builtin_authenticator_functions::PublicKeyFieldName,
        move_public_key::MovePublicKey,
        object::{MoveStructExt, ObjectInner},
    };
    use test_cluster::smart_account::ed25519_key;

    use super::*;

    fn id(hex: &str) -> ObjectId {
        ObjectId::from_short_hex(hex).unwrap()
    }

    fn object(object_type: StructTag, owner: Owner, contents: Vec<u8>) -> Object {
        let data = ObjectData::Struct(
            MoveStruct::new_from_execution_with_limit(object_type, 1.into(), contents, 256)
                .unwrap(),
        );
        ObjectInner {
            owner,
            data,
            previous_transaction: TransactionDigest::GENESIS_MARKER,
            storage_rebate: 0,
        }
        .into()
    }

    fn parent(object_type: StructTag, object_id: ObjectId) -> Object {
        object(
            object_type,
            Owner::Shared(1.into()),
            bcs::to_bytes(&UID::new(object_id)).unwrap(),
        )
    }

    fn key_field(object_id: ObjectId, parent_id: ObjectId) -> Object {
        let field = Field {
            id: UID::new(object_id),
            name: PublicKeyFieldName::default(),
            value: MovePublicKey::from(&ed25519_key(1)),
        };
        object(
            public_key_field_tag(),
            Owner::Object(parent_id),
            bcs::to_bytes(&field).unwrap(),
        )
    }

    /// The key fields arrive in one partition and their parents in a later
    /// one. Only the field whose parent is a `SmartAccount` gets a row.
    #[test]
    fn keys_are_matched_with_parents_from_a_later_partition() {
        let holder_type = StructTag::new(
            Address::from_short_hex("0xe7").unwrap(),
            Identifier::from_static("holder"),
            Identifier::from_static("Holder"),
            vec![],
        );
        let account_key = key_field(id("0x1a"), id("0xf3"));
        let holder_key = key_field(id("0x2c"), id("0xe0"));
        let account = parent(smart_account_tag(), id("0xf3"));
        let holder = parent(holder_type, id("0xe0"));

        let mut first_partition = SmartAccountKeys::default();
        first_partition
            .add_objects([&account_key, &holder_key])
            .unwrap();
        let mut second_partition = SmartAccountKeys::default();
        second_partition.add_objects([&holder, &account]).unwrap();

        let mut collected = SmartAccountKeys::default();
        collected.append(first_partition);
        collected.append(second_partition);

        assert_eq!(
            collected.into_rows(),
            vec![StoredSmartAccountKey::from_key_field(&account_key).unwrap()]
        );
    }
}
