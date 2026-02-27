// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use async_graphql::{
    connection::{Connection, ConnectionNameType, CursorType, Edge, EdgeNameType, EmptyFields},
    *,
};
use iota_indexer::models::transactions::{OptimisticTransaction, StoredTransaction};
use iota_json_rpc_types::IotaExecutionStatus;
use iota_types::{
    effects::{TransactionEffects as NativeTransactionEffects, TransactionEffectsAPI},
    event::Event as NativeEvent,
    execution_status::ExecutionStatus as NativeExecutionStatus,
    transaction::TransactionData as NativeTransactionData,
};

use crate::{
    config::DEFAULT_PAGE_SIZE,
    consistency::{ConsistentIndexCursor, UNAVAILABLE_CHECKPOINT_SEQUENCE_NUMBER},
    data::package_resolver::PackageResolver,
    error::Error,
    types::{
        balance_change::BalanceChange,
        base64::Base64,
        checkpoint::{Checkpoint, CheckpointId},
        cursor::{JsonCursor, Page},
        date_time::DateTime,
        digest::Digest,
        epoch::Epoch,
        event::Event,
        gas::GasEffects,
        object_change::{ObjectChange, ObjectChangeSource},
        transaction_block::{TransactionBlock, TransactionBlockInner},
        uint53::UInt53,
        unchanged_shared_object::UnchangedSharedObject,
    },
};

/// Wraps the actual transaction block effects data with the checkpoint sequence
/// number at which the data was viewed, for consistent results on paginating
/// through and resolving nested types.
#[derive(Clone, Debug)]
pub(crate) struct TransactionBlockEffects {
    pub kind: TransactionBlockEffectsKind,
    /// The checkpoint sequence number this was viewed at.
    pub checkpoint_viewed_at: u64,
}

#[derive(Clone, Debug)]
pub(crate) enum TransactionBlockEffectsKind {
    /// A transaction that has been checkpointed and stored in the database,
    /// containing all information that the other two variants have, and more.
    Checkpointed {
        stored_tx: StoredTransaction,
        native: NativeTransactionEffects,
    },
    /// A transaction block that has been executed and indexed without
    /// checkpoint information.
    Executed {
        optimistic_tx: OptimisticTransaction,
        native: NativeTransactionEffects,
    },

    /// A transaction block that has been executed via dryRunTransactionBlock.
    /// Similar to Executed, it does not contain checkpoint, timestamp or
    /// balanceChanges.
    DryRun {
        tx_data: NativeTransactionData,
        native: NativeTransactionEffects,
        events: Vec<NativeEvent>,
    },
}

/// The execution status of this transaction block: success or failure.
#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum ExecutionStatus {
    /// The transaction block was successfully executed
    Success,
    /// The transaction block could not be executed
    Failure,
}

/// Type to override names of the Dependencies Connection (which has nullable
/// transactions and therefore must be a different types to the default
/// `TransactionBlockConnection`).
struct DependencyConnectionNames;

type CDependencies = JsonCursor<ConsistentIndexCursor>;
type CUnchangedSharedObject = JsonCursor<ConsistentIndexCursor>;
type CObjectChange = JsonCursor<ConsistentIndexCursor>;
type CBalanceChange = JsonCursor<ConsistentIndexCursor>;
type CEvent = JsonCursor<ConsistentIndexCursor>;

/// The effects representing the result of executing a transaction block.
#[Object]
impl TransactionBlockEffects {
    /// The transaction that ran to produce these effects.
    #[graphql(complexity = "child_complexity")]
    async fn transaction_block(&self) -> Result<Option<TransactionBlock>> {
        Ok(Some(self.clone().try_into().extend()?))
    }

    /// Whether the transaction executed successfully or not.
    #[graphql(complexity = 0)]
    async fn status(&self) -> Option<ExecutionStatus> {
        Some(match self.native().status() {
            NativeExecutionStatus::Success => ExecutionStatus::Success,
            NativeExecutionStatus::Failure { .. } => ExecutionStatus::Failure,
        })
    }

    /// The latest version of all objects (apart from packages) that have been
    /// created or modified by this transaction, immediately following this
    /// transaction.
    #[graphql(complexity = 0)]
    async fn lamport_version(&self) -> UInt53 {
        self.native().lamport_version().value().into()
    }

    /// The reason for a transaction failure, if it did fail.
    /// If the error is a Move abort, the error message will be resolved to a
    /// human-readable form if possible, otherwise it will fall back to
    /// displaying the abort code and location.
    #[graphql(complexity = 0)]
    async fn errors(&self, ctx: &Context<'_>) -> Result<Option<String>> {
        let resolver: &PackageResolver = ctx.data_unchecked();

        let status = IotaExecutionStatus::from_native_with_clever_error(
            self.native().status().clone(),
            resolver,
        )
        .await;
        match status {
            IotaExecutionStatus::Success => Ok(None),
            IotaExecutionStatus::Failure { error } => Ok(Some(error)),
        }
    }

    /// Transactions whose outputs this transaction depends upon.
    #[graphql(complexity = "child_complexity")]
    async fn dependencies(
        &self,
        ctx: &Context<'_>,
        first: Option<u64>,
        after: Option<CDependencies>,
        last: Option<u64>,
        before: Option<CDependencies>,
    ) -> Result<
        Connection<
            String,
            Option<TransactionBlock>,
            EmptyFields,
            EmptyFields,
            DependencyConnectionNames,
            DependencyConnectionNames,
        >,
    > {
        let page = Page::from_params(ctx.data_unchecked(), first, after, last, before)?;
        let mut connection = Connection::new(false, false);

        let dependencies = self.native().dependencies();

        let Some(consistent_page) =
            page.paginate_consistent_indices(dependencies.len(), self.checkpoint_viewed_at)?
        else {
            return Ok(connection);
        };

        let indices: Vec<CDependencies> = consistent_page.cursors.collect();

        let (Some(fst), Some(lst)) = (indices.first(), indices.last()) else {
            return Ok(connection);
        };

        let transactions = TransactionBlock::multi_query(
            ctx,
            dependencies[fst.ix..=lst.ix]
                .iter()
                .map(|d| Digest::from(*d))
                .collect(),
            fst.c, // Each element's cursor has the same checkpoint sequence number set
        )
        .await
        .extend()?;

        if transactions.is_empty() {
            return Ok(connection);
        };

        connection.has_previous_page = consistent_page.has_previous_page;
        connection.has_next_page = consistent_page.has_next_page;

        for c in indices {
            let digest: Digest = dependencies[c.ix].into();
            connection.edges.push(Edge::new(
                c.encode_cursor(),
                transactions.get(&digest).cloned(),
            ));
        }

        Ok(connection)
    }

    /// Effects to the gas object.
    #[graphql(complexity = "child_complexity")]
    async fn gas_effects(&self) -> Option<GasEffects> {
        Some(GasEffects::from(self.native(), self.checkpoint_viewed_at))
    }

    /// Shared objects that are referenced by but not changed by this
    /// transaction.
    #[graphql(complexity = "child_complexity")]
    async fn unchanged_shared_objects(
        &self,
        ctx: &Context<'_>,
        first: Option<u64>,
        after: Option<CUnchangedSharedObject>,
        last: Option<u64>,
        before: Option<CUnchangedSharedObject>,
    ) -> Result<Connection<String, UnchangedSharedObject>> {
        let page = Page::from_params(ctx.data_unchecked(), first, after, last, before)?;
        let mut connection = Connection::new(false, false);

        let input_shared_objects = self.native().input_shared_objects();

        let Some(consistent_page) = page
            .paginate_consistent_indices(input_shared_objects.len(), self.checkpoint_viewed_at)?
        else {
            return Ok(connection);
        };

        connection.has_previous_page = consistent_page.has_previous_page;
        connection.has_next_page = consistent_page.has_next_page;

        for c in consistent_page.cursors {
            let result = UnchangedSharedObject::try_from(input_shared_objects[c.ix].clone(), c.c);
            match result {
                Ok(unchanged_shared_object) => {
                    connection
                        .edges
                        .push(Edge::new(c.encode_cursor(), unchanged_shared_object));
                }
                Err(_shared_object_changed) => continue, /* Only add unchanged shared objects to
                                                          * the connection. */
            }
        }

        Ok(connection)
    }

    /// The effect this transaction had on objects on-chain.
    #[graphql(complexity = "child_complexity")]
    async fn object_changes(
        &self,
        ctx: &Context<'_>,
        first: Option<u64>,
        after: Option<CObjectChange>,
        last: Option<u64>,
        before: Option<CObjectChange>,
    ) -> Result<Connection<String, ObjectChange>> {
        let page = Page::from_params(ctx.data_unchecked(), first, after, last, before)?;
        let mut connection = Connection::new(false, false);

        let object_changes = self.native().object_changes();

        let Some(consistent_page) =
            page.paginate_consistent_indices(object_changes.len(), self.checkpoint_viewed_at)?
        else {
            return Ok(connection);
        };

        connection.has_previous_page = consistent_page.has_previous_page;
        connection.has_next_page = consistent_page.has_next_page;

        // Determine the source based on the transaction block effects kind
        let source = match &self.kind {
            TransactionBlockEffectsKind::Checkpointed { .. } => ObjectChangeSource::Checkpointed,
            TransactionBlockEffectsKind::Executed { .. } => ObjectChangeSource::Executed,
            TransactionBlockEffectsKind::DryRun { .. } => ObjectChangeSource::DryRun,
        };

        for c in consistent_page.cursors {
            let object_change = ObjectChange {
                native: object_changes[c.ix].clone(),
                checkpoint_viewed_at: c.c,
                source: source.clone(),
            };

            connection
                .edges
                .push(Edge::new(c.encode_cursor(), object_change));
        }

        Ok(connection)
    }

    /// The effect this transaction had on the balances (sum of coin values per
    /// coin type) of addresses and objects.
    #[graphql(complexity = "child_complexity")]
    async fn balance_changes(
        &self,
        ctx: &Context<'_>,
        first: Option<u64>,
        after: Option<CBalanceChange>,
        last: Option<u64>,
        before: Option<CBalanceChange>,
    ) -> Result<Connection<String, BalanceChange>> {
        let page = Page::from_params(ctx.data_unchecked(), first, after, last, before)?;
        let mut connection = Connection::new(false, false);

        let balance_len = match &self.kind {
            TransactionBlockEffectsKind::Checkpointed { stored_tx, .. } => {
                stored_tx.get_balance_len()
            }
            TransactionBlockEffectsKind::Executed { optimistic_tx, .. } => {
                optimistic_tx.get_balance_len()
            }
            // DryRun variant doesn't have balance changes available
            _ => return Ok(connection),
        };

        let Some(consistent_page) =
            page.paginate_consistent_indices(balance_len, self.checkpoint_viewed_at)?
        else {
            return Ok(connection);
        };

        connection.has_previous_page = consistent_page.has_previous_page;
        connection.has_next_page = consistent_page.has_next_page;

        for c in consistent_page.cursors {
            let serialized = match &self.kind {
                TransactionBlockEffectsKind::Checkpointed { stored_tx, .. } => {
                    stored_tx.get_balance_at_idx(c.ix)
                }
                TransactionBlockEffectsKind::Executed { optimistic_tx, .. } => {
                    optimistic_tx.get_balance_at_idx(c.ix)
                }
                _ => None,
            };

            let Some(serialized) = serialized else {
                continue;
            };

            let balance_change = BalanceChange::read(&serialized, c.c).extend()?;
            connection
                .edges
                .push(Edge::new(c.encode_cursor(), balance_change));
        }

        Ok(connection)
    }

    /// Events emitted by this transaction block.
    #[graphql(
        complexity = "first.or(last).unwrap_or(DEFAULT_PAGE_SIZE as u64) as usize * child_complexity"
    )]
    async fn events(
        &self,
        ctx: &Context<'_>,
        first: Option<u64>,
        after: Option<CEvent>,
        last: Option<u64>,
        before: Option<CEvent>,
    ) -> Result<Connection<String, Event>> {
        let page = Page::from_params(ctx.data_unchecked(), first, after, last, before)?;
        let mut connection = Connection::new(false, false);
        let len = match &self.kind {
            TransactionBlockEffectsKind::Checkpointed { stored_tx, .. } => {
                stored_tx.get_event_len()
            }
            TransactionBlockEffectsKind::Executed { optimistic_tx, .. } => {
                optimistic_tx.get_event_len()
            }
            TransactionBlockEffectsKind::DryRun { events, .. } => events.len(),
        };
        let Some(consistent_page) =
            page.paginate_consistent_indices(len, self.checkpoint_viewed_at)?
        else {
            return Ok(connection);
        };

        connection.has_previous_page = consistent_page.has_previous_page;
        connection.has_next_page = consistent_page.has_next_page;

        for c in consistent_page.cursors {
            let event = match &self.kind {
                TransactionBlockEffectsKind::Checkpointed { stored_tx, .. } => {
                    Event::try_from_stored_transaction(stored_tx, c.ix, c.c).extend()?
                }
                TransactionBlockEffectsKind::Executed { optimistic_tx, .. } => {
                    Event::try_from_optimistic_transaction(optimistic_tx, c.ix, c.c).extend()?
                }
                TransactionBlockEffectsKind::DryRun { events, .. } => Event {
                    checkpointed_info: None,
                    native: events[c.ix].clone(),
                    checkpoint_viewed_at: c.c,
                },
            };
            connection.edges.push(Edge::new(c.encode_cursor(), event));
        }

        Ok(connection)
    }

    /// Timestamp corresponding to the checkpoint this transaction was finalized
    /// in.
    #[graphql(complexity = 0)]
    async fn timestamp(&self) -> Result<Option<DateTime>, Error> {
        let TransactionBlockEffectsKind::Checkpointed { stored_tx, .. } = &self.kind else {
            return Ok(None);
        };
        Ok(Some(DateTime::from_ms(stored_tx.timestamp_ms)?))
    }

    /// The epoch this transaction was executed in.
    #[graphql(complexity = "child_complexity")]
    async fn epoch(&self, ctx: &Context<'_>) -> Result<Option<Epoch>> {
        Epoch::query(
            ctx,
            Some(self.native().executed_epoch()),
            self.checkpoint_viewed_at,
        )
        .await
        .extend()
    }

    /// The checkpoint this transaction was finalized in, if it is within the
    /// available range.
    #[graphql(complexity = "child_complexity")]
    async fn checkpoint(&self, ctx: &Context<'_>) -> Result<Option<Checkpoint>> {
        // If the transaction data is not a checkpointed transaction, it's not in the
        // checkpoint yet so we return None.
        let TransactionBlockEffectsKind::Checkpointed { stored_tx, .. } = &self.kind else {
            return Ok(None);
        };
        if !self.is_available() {
            return Ok(None);
        }

        Checkpoint::query(
            ctx,
            CheckpointId::by_seq_num(stored_tx.checkpoint_sequence_number as u64),
            self.checkpoint_viewed_at,
        )
        .await
        .extend()
    }

    /// Base64 encoded bcs serialization of the on-chain transaction effects.
    #[graphql(complexity = 0)]
    async fn bcs(&self) -> Result<Base64> {
        let bytes = match &self.kind {
            TransactionBlockEffectsKind::Checkpointed { stored_tx, .. } => {
                stored_tx.raw_effects.clone()
            }
            TransactionBlockEffectsKind::Executed { optimistic_tx, .. } => {
                optimistic_tx.raw_effects.clone()
            }
            _ => bcs::to_bytes(&self.native())
                .map_err(|e| Error::Internal(format!("Error serializing transaction effects: {e}")))
                .extend()?,
        };

        Ok(Base64::from(bytes))
    }
}

impl TransactionBlockEffects {
    fn native(&self) -> &NativeTransactionEffects {
        match &self.kind {
            TransactionBlockEffectsKind::Checkpointed { native, .. } => native,
            TransactionBlockEffectsKind::DryRun { native, .. } => native,
            TransactionBlockEffectsKind::Executed { native, .. } => native,
        }
    }

    /// Returns whether the parent transaction is within the available range.
    pub(crate) fn is_available(&self) -> bool {
        self.checkpoint_viewed_at < UNAVAILABLE_CHECKPOINT_SEQUENCE_NUMBER
    }
}

impl ConnectionNameType for DependencyConnectionNames {
    fn type_name<T: OutputType>() -> String {
        "DependencyConnection".to_string()
    }
}

impl EdgeNameType for DependencyConnectionNames {
    fn type_name<T: OutputType>() -> String {
        "DependencyEdge".to_string()
    }
}

impl TryFrom<OptimisticTransaction> for TransactionBlockEffectsKind {
    type Error = Error;

    fn try_from(optimistic_tx: OptimisticTransaction) -> Result<Self, Error> {
        let native = bcs::from_bytes(&optimistic_tx.raw_effects).map_err(|e| {
            Error::Internal(format!(
                "Failed to deserialize NativeTransactionEffects from optimistic transaction: {e}"
            ))
        })?;

        Ok(TransactionBlockEffectsKind::Executed {
            optimistic_tx,
            native,
        })
    }
}

impl TryFrom<OptimisticTransaction> for TransactionBlockEffects {
    type Error = Error;

    fn try_from(tx: OptimisticTransaction) -> Result<Self, Error> {
        // set to u64::MAX, as the executed transaction has not been indexed yet
        let checkpoint_viewed_at = u64::MAX;
        Ok(Self {
            kind: tx.try_into()?,
            checkpoint_viewed_at,
        })
    }
}

impl TryFrom<TransactionBlock> for TransactionBlockEffectsKind {
    type Error = Error;

    fn try_from(block: TransactionBlock) -> Result<Self, Error> {
        match block.inner {
            TransactionBlockInner::Checkpointed { stored_tx, .. } => {
                bcs::from_bytes(&stored_tx.raw_effects)
                    .map(|native| TransactionBlockEffectsKind::Checkpointed { stored_tx, native })
                    .map_err(|e| {
                        Error::Internal(format!("Error deserializing transaction effects: {e}"))
                    })
            }
            TransactionBlockInner::Executed { optimistic_tx, .. } => {
                TransactionBlockEffectsKind::try_from(optimistic_tx)
            }

            TransactionBlockInner::DryRun {
                tx_data,
                effects,
                events,
            } => Ok(TransactionBlockEffectsKind::DryRun {
                tx_data,
                native: effects,
                events,
            }),
        }
    }
}

impl TryFrom<TransactionBlock> for TransactionBlockEffects {
    type Error = Error;

    fn try_from(block: TransactionBlock) -> Result<Self, Error> {
        let checkpoint_viewed_at = block.checkpoint_viewed_at;
        Ok(Self {
            kind: block.try_into()?,
            checkpoint_viewed_at,
        })
    }
}
