// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_json_rpc_types::{EventFilter, TransactionFilter, TransactionFilterV2};
use iota_types::messages_checkpoint::CheckpointSequenceNumber;

use crate::{
    errors::{IndexerError, IndexerResult},
    pruning::CommitterTables,
    read::{IndexerReader, TransactionFilterKind},
};

/// The oldest checkpoint the indexer still has data for, for a given kind of request, resolved from
/// the watermarks of the tables that request reads.
///
/// There is one constructor per kind of request, e.g. a transaction by its
/// digest or the transactions of an address. Each names the tables the request
/// reads and whether the historical fallback can serve it.
///
/// Two values are kept: the checkpoint the database alone has data from, and the checkpoint the
/// indexer as a whole has data from (db + fallback). The former can be used internally to e.g.
/// decide whether fallback needs to be consulted, the latter is the value reported to the user.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OldestAvailableCheckpoint {
    /// The oldest checkpoint the database alone has data for.
    db_checkpoint: CheckpointSequenceNumber,
    /// The transaction `db_checkpoint` starts at.
    db_tx: i64,
    /// The oldest checkpoint the indexer as a whole has data for. Equal to
    /// `db_checkpoint`, except that tables the fallback serves no longer
    /// count once a fallback is configured.
    served_checkpoint: CheckpointSequenceNumber,
}

impl OldestAvailableCheckpoint {
    /// A checkpoint looked up by sequence number or digest, or a page of
    /// checkpoints.
    pub fn checkpoint(reader: &IndexerReader) -> Self {
        Self::for_tables(reader, &[], &[CommitterTables::Checkpoints])
    }

    /// A transaction looked up by its digest, or the events of a single transaction.
    pub fn transaction_by_digest(reader: &IndexerReader) -> Self {
        Self::for_tables(
            reader,
            &[],
            &[
                CommitterTables::Transactions,
                CommitterTables::TxGlobalOrder,
            ],
        )
    }

    /// The transactions of a checkpoint.
    pub fn transactions_by_checkpoint(reader: &IndexerReader) -> Self {
        Self::for_tables(
            reader,
            &[],
            &[
                CommitterTables::Transactions,
                CommitterTables::PrunerCpWatermark,
                CommitterTables::TxGlobalOrder,
            ],
        )
    }

    /// The transactions sent to or received by an address.
    pub fn transactions_by_address(reader: &IndexerReader) -> Self {
        Self::for_tables(
            reader,
            &[],
            &[
                CommitterTables::TxSenders,
                CommitterTables::TxRecipients,
                CommitterTables::Transactions,
                CommitterTables::TxGlobalOrder,
            ],
        )
    }

    /// The transactions matching a filter. Each filter reads its
    /// lookup table (`tx_*`) and `tx_global_order` for the cursor digest, and the
    /// transaction contents from `transactions` unless a fallback is
    /// configured. The checkpoint and affected-address filters are special cases and have their own
    /// fallback paths.
    ///
    /// # Errors
    ///
    /// Fails for a filter the indexer does not support.
    pub(crate) fn transactions_by_filter(
        reader: &IndexerReader,
        filter: Option<&TransactionFilterKind>,
    ) -> IndexerResult<Self> {
        use CommitterTables::*;

        let Some(filter) = filter else {
            return Ok(Self::for_tables(reader, &[Transactions], &[]));
        };
        let lookup_tables: &[CommitterTables] = match filter {
            TransactionFilterKind::V1(TransactionFilter::Checkpoint(_))
            | TransactionFilterKind::V2(TransactionFilterV2::Checkpoint(_)) => {
                return Ok(Self::transactions_by_checkpoint(reader));
            }
            TransactionFilterKind::V1(TransactionFilter::FromOrToAddress { .. })
            | TransactionFilterKind::V2(TransactionFilterV2::FromOrToAddress { .. }) => {
                return Ok(Self::transactions_by_address(reader));
            }
            TransactionFilterKind::V1(TransactionFilter::MoveFunction {
                module, function, ..
            })
            | TransactionFilterKind::V2(TransactionFilterV2::MoveFunction {
                module,
                function,
                ..
            }) => match (module, function) {
                (Some(_), Some(_)) => &[TxCallsFun, TxGlobalOrder],
                (Some(_), None) => &[TxCallsMod, TxGlobalOrder],
                (None, Some(_)) => {
                    return Err(IndexerError::InvalidArgument(
                        "Function cannot be present without Module.".into(),
                    ));
                }
                (None, None) => &[TxCallsPkg, TxGlobalOrder],
            },
            TransactionFilterKind::V1(TransactionFilter::InputObject(_))
            | TransactionFilterKind::V2(TransactionFilterV2::InputObject(_)) => {
                &[TxInputObjects, TxGlobalOrder]
            }
            TransactionFilterKind::V1(TransactionFilter::ChangedObject(_))
            | TransactionFilterKind::V2(TransactionFilterV2::ChangedObject(_)) => {
                &[TxChangedObjects, TxGlobalOrder]
            }
            TransactionFilterKind::V2(TransactionFilterV2::WrappedOrDeletedObject(_)) => {
                &[TxWrappedOrDeletedObjects, TxGlobalOrder]
            }
            TransactionFilterKind::V1(TransactionFilter::FromAddress(_))
            | TransactionFilterKind::V2(TransactionFilterV2::FromAddress(_)) => {
                &[TxSenders, TxGlobalOrder]
            }
            TransactionFilterKind::V1(TransactionFilter::ToAddress(_))
            | TransactionFilterKind::V2(TransactionFilterV2::ToAddress(_)) => {
                &[TxRecipients, TxGlobalOrder]
            }
            TransactionFilterKind::V1(TransactionFilter::FromAndToAddress { .. })
            | TransactionFilterKind::V2(TransactionFilterV2::FromAndToAddress { .. }) => {
                &[TxSenders, TxRecipients, TxGlobalOrder]
            }
            TransactionFilterKind::V1(TransactionFilter::TransactionKind(_))
            | TransactionFilterKind::V2(TransactionFilterV2::TransactionKind(_))
            | TransactionFilterKind::V1(TransactionFilter::TransactionKindIn(_))
            | TransactionFilterKind::V2(TransactionFilterV2::TransactionKindIn(_)) => {
                &[TxKinds, TxGlobalOrder]
            }
            // `TransactionFilterV2` is `#[non_exhaustive]`: a variant added later is rejected here.
            TransactionFilterKind::V2(_) => {
                return Err(IndexerError::InvalidArgument(
                    "transaction filter is not supported".into(),
                ));
            }
        };
        // The transaction contents come from `transactions` unless a fallback is
        // configured.
        let mut tables = lookup_tables.to_vec();
        if !reader.is_fallback_enabled() {
            tables.push(Transactions);
        }
        Ok(Self::for_tables(reader, &tables, &[]))
    }

    /// The events of a transaction looked up by its digest.
    pub fn events_by_tx_digest(reader: &IndexerReader) -> Self {
        Self::for_tables(
            reader,
            &[],
            &[CommitterTables::Events, CommitterTables::TxGlobalOrder],
        )
    }

    /// The events matching a filter. Each filter reads `events` and
    /// `tx_global_order` for the cursor digest, and the sender filter `tx_senders`
    /// as well. The transaction filter is a special case and has its own fallback
    /// path.
    ///
    /// # Errors
    ///
    /// Fails for a filter the indexer does not support.
    pub fn events_by_filter(reader: &IndexerReader, filter: &EventFilter) -> IndexerResult<Self> {
        use CommitterTables::*;

        let tables: &[CommitterTables] = match filter {
            EventFilter::Transaction(_) => return Ok(Self::events_by_tx_digest(reader)),
            EventFilter::Sender(_) => &[Events, TxSenders, TxGlobalOrder],
            EventFilter::Package(_)
            | EventFilter::MoveModule { .. }
            | EventFilter::MoveEventType(_)
            | EventFilter::MoveEventModule { .. } => &[Events, TxGlobalOrder],
            EventFilter::MoveEventField { .. }
            | EventFilter::All(_)
            | EventFilter::Any(_)
            | EventFilter::And(_, _)
            | EventFilter::Or(_, _)
            | EventFilter::TimeRange { .. } => {
                return Err(IndexerError::NotSupported(
                    "This type of EventFilter is not supported.".into(),
                ));
            }
        };
        Ok(Self::for_tables(reader, tables, &[]))
    }

    /// Whether an object version exists, from `objects_version`.
    pub fn past_object_version(reader: &IndexerReader) -> Self {
        Self::for_tables(reader, &[], &[CommitterTables::ObjectsVersion])
    }

    /// The content of a superseded object version, from
    /// `objects_backward_history`.
    pub fn past_object_content(reader: &IndexerReader) -> Self {
        Self::for_tables(reader, &[], &[CommitterTables::ObjectsBackwardHistory])
    }

    /// A request reading `db_tables` from the database only, and
    /// `fallback_tables` from the database or the fallback when one is
    /// configured. Use this for a request without a constructor of its own.
    ///
    /// A table absent from the cache has not been written yet, and so has not
    /// been pruned either: it is treated as 0. This relies on the cache being filled before the RPC
    /// server starts serving.
    pub fn for_tables(
        reader: &IndexerReader,
        db_tables: &[CommitterTables],
        fallback_tables: &[CommitterTables],
    ) -> Self {
        let watermarks = reader.watermark_cache().load();
        let lowest = |tables: &[CommitterTables]| {
            watermarks
                .lowest_available_cp_and_tx(tables)
                .unwrap_or((0, 0))
        };
        let all_tables: Vec<CommitterTables> =
            db_tables.iter().chain(fallback_tables).cloned().collect();
        let (db_checkpoint, db_tx) = lowest(&all_tables);
        // The fallback serves `fallback_tables` from genesis on.
        let served_tables = if reader.is_fallback_enabled() {
            db_tables
        } else {
            all_tables.as_slice()
        };
        let (served_checkpoint, _) = lowest(served_tables);
        Self {
            db_checkpoint: db_checkpoint as CheckpointSequenceNumber,
            db_tx,
            served_checkpoint: served_checkpoint as CheckpointSequenceNumber,
        }
    }

    /// The oldest checkpoint the database serves the lookup from.
    pub fn db_checkpoint(self) -> CheckpointSequenceNumber {
        self.db_checkpoint
    }

    /// The oldest transaction the database serves the lookup from.
    pub fn db_tx(self) -> i64 {
        self.db_tx
    }

    /// Whether the indexer serves the lookup from genesis on.
    pub fn has_full_history(self) -> bool {
        self.served_checkpoint == 0
    }

    /// Whether the database serves the lookup from genesis on.
    pub fn db_has_full_history(self) -> bool {
        self.db_checkpoint == 0
    }

    /// The later of the two, for a lookup that reads both `self` and `other`.
    pub fn max(self, other: Self) -> Self {
        Self {
            db_checkpoint: self.db_checkpoint.max(other.db_checkpoint),
            db_tx: self.db_tx.max(other.db_tx),
            served_checkpoint: self.served_checkpoint.max(other.served_checkpoint),
        }
    }

    /// Fails with [`IndexerError::DataPruned`] when the database no longer
    /// serves `checkpoint_seq`.
    pub fn ensure_db_serves_checkpoint(self, checkpoint_seq: u64) -> IndexerResult<()> {
        if checkpoint_seq >= self.db_checkpoint {
            return Ok(());
        }
        Err(IndexerError::DataPruned {
            message: format!(
                "checkpoint {checkpoint_seq} has been pruned (min available: {})",
                self.db_checkpoint
            ),
            oldest_available_checkpoint: self,
        })
    }

    /// Fails with [`IndexerError::DataPruned`] when the database no longer
    /// serves `tx_seq`.
    pub fn ensure_db_serves_tx(self, tx_seq: i64) -> IndexerResult<()> {
        if tx_seq >= self.db_tx {
            return Ok(());
        }
        Err(IndexerError::DataPruned {
            message: format!(
                "transaction {tx_seq} has been pruned (min available: {})",
                self.db_tx
            ),
            oldest_available_checkpoint: self,
        })
    }
}

/// The oldest checkpoint the indexer serves the request from, using the
/// fallback if available: the value reported to the user.
impl From<OldestAvailableCheckpoint> for CheckpointSequenceNumber {
    fn from(oldest_available: OldestAvailableCheckpoint) -> Self {
        oldest_available.served_checkpoint
    }
}
