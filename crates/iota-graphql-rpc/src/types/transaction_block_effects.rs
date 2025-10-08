// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use async_graphql::{
    connection::{Connection, ConnectionNameType, CursorType, Edge, EdgeNameType, EmptyFields},
    *,
};
use fastcrypto::encoding::{Base64 as FBase64, Encoding};
use iota_indexer::models::transactions::{OptimisticTransaction, StoredTransaction};
use iota_json_rpc_types::IotaTransactionBlockEffectsAPI;
use iota_json_rpc_types::{
    IotaExecutionStatus, IotaTransactionBlockEffects, ObjectChange as RpcObjectChange,
};
use iota_package_resolver::{CleverError, ErrorConstants};
use iota_types::{
    effects::{
        ObjectChange as NativeObjectChange, TransactionEffects as NativeTransactionEffects,
        TransactionEffectsAPI,
    },
    event::Event as NativeEvent,
    execution_status::{
        ExecutionFailureStatus, ExecutionStatus as NativeExecutionStatus, MoveLocation,
        MoveLocationOpt,
    },
    transaction::{
        Command, ProgrammableTransaction, SenderSignedData as NativeSenderSignedData,
        TransactionData as NativeTransactionData, TransactionDataAPI,
        TransactionKind as NativeTransactionKind,
    },
};

use crate::{
    consistency::ConsistentIndexCursor,
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
        rpc: IotaTransactionBlockEffects,
    },
    /// A transaction block that has been executed and indexed without
    /// checkpoint information.
    Executed {
        optimistic_tx: OptimisticTransaction,
        native: NativeTransactionEffects,
        rpc: IotaTransactionBlockEffects,
    },

    /// A transaction block that has been executed via dryRunTransactionBlock.
    /// Similar to Executed, it does not contain checkpoint, timestamp or
    /// balanceChanges.
    DryRun {
        tx_data: NativeTransactionData,
        native_effects: Option<NativeTransactionEffects>,
        rpc: IotaTransactionBlockEffects,
        rpc_object_changes: Vec<RpcObjectChange>,
        events: Vec<NativeEvent>,
        balance_changes: Vec<Vec<u8>>,
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
    async fn transaction_block(&self) -> Result<Option<TransactionBlock>> {
        Ok(Some(self.clone().try_into().extend()?))
    }

    /// Whether the transaction executed successfully or not.
    async fn status(&self) -> Option<ExecutionStatus> {
        let rpc = self.rpc();
        Some(match rpc.status() {
            IotaExecutionStatus::Success => ExecutionStatus::Success,
            IotaExecutionStatus::Failure { .. } => ExecutionStatus::Failure,
        })
    }

    /// The latest version of all objects (apart from packages) that have been
    /// created or modified by this transaction, immediately following this
    /// transaction.
    async fn lamport_version(&self) -> UInt53 {
        let rpc = self.rpc();
        let lamport_version = rpc
            .created()
            .iter()
            .chain(rpc.mutated().iter())
            .map(|obj| obj.reference.version)
            .max()
            .unwrap_or_else(|| 1.into());
        lamport_version.value().into()
    }

    /// The reason for a transaction failure, if it did fail.
    /// If the error is a Move abort, the error message will be resolved to a
    /// human-readable form if possible, otherwise it will fall back to
    /// displaying the abort code and location.
    async fn errors(&self, ctx: &Context<'_>) -> Result<Option<String>> {
        let resolver: &PackageResolver = ctx.data_unchecked();

        // Check if native() is available for detailed error resolution
        let status = if self.native().is_some() {
            // Use the full resolve_native_status_impl for detailed error information
            self.resolve_native_status_impl(resolver).await?
        } else {
            // Fall back to simple error string from rpc() when native() is None
            match self.rpc().status() {
                IotaExecutionStatus::Success => {
                    return Ok(None);
                }
                IotaExecutionStatus::Failure { error } => {
                    return Ok(Some(error.clone()));
                }
            }
        };

        match status {
            NativeExecutionStatus::Success => Ok(None),

            NativeExecutionStatus::Failure {
                error,
                command: None,
            } => Ok(Some(error.to_string())),

            NativeExecutionStatus::Failure {
                error,
                command: Some(command),
            } => {
                let error = 'error: {
                    let ExecutionFailureStatus::MoveAbort(loc, code) = &error else {
                        break 'error error.to_string();
                    };
                    let fname_string = if let Some(fname) = &loc.function_name {
                        format!("::{fname}'")
                    } else {
                        "'".to_string()
                    };

                    let Some(CleverError {
                        module_id,
                        source_line_number,
                        error_info,
                    }) = resolver
                        .resolve_clever_error(loc.module.clone(), *code)
                        .await
                    else {
                        break 'error format!(
                            "from '{}{fname_string} (instruction {}), abort code: {code}",
                            loc.module.to_canonical_display(true),
                            loc.instruction,
                        );
                    };

                    match error_info {
                        ErrorConstants::Rendered {
                            identifier,
                            constant,
                        } => {
                            format!(
                                "from '{}{fname_string} (line {source_line_number}), abort '{identifier}': {constant}",
                                module_id.to_canonical_display(true)
                            )
                        }
                        ErrorConstants::Raw { identifier, bytes } => {
                            let const_str = FBase64::encode(bytes);
                            format!(
                                "from '{}{fname_string} (line {source_line_number}), abort '{identifier}': {const_str}",
                                module_id.to_canonical_display(true)
                            )
                        }
                        ErrorConstants::None => {
                            format!(
                                "from '{}{fname_string} (line {source_line_number})",
                                module_id.to_canonical_display(true)
                            )
                        }
                    }
                };
                // Convert the command index into an ordinal.
                let command = command + 1;
                let suffix = match command % 10 {
                    1 => "st",
                    2 => "nd",
                    3 => "rd",
                    _ => "th",
                };
                Ok(Some(format!("Error in {command}{suffix} command, {error}")))
            }
        }
    }

    /// Transactions whose outputs this transaction depends upon.
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

        let rpc = self.rpc();
        let dependencies: Vec<_> = rpc.dependencies().to_vec();

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
    async fn gas_effects(&self) -> Option<GasEffects> {
        Some(GasEffects::from_rpc_effects(
            self.rpc(),
            self.checkpoint_viewed_at,
        ))
    }

    /// Shared objects that are referenced by but not changed by this
    /// transaction.
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

        let input_shared_objects: Vec<_> = match self.native() {
            Some(native) => native.input_shared_objects(),
            None => return Ok(connection),
        };

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

        let object_changes: Vec<_> = match &self.kind {
            TransactionBlockEffectsKind::Checkpointed { native, .. }
            | TransactionBlockEffectsKind::Executed { native, .. } => native.object_changes(),
            TransactionBlockEffectsKind::DryRun {
                native_effects,
                rpc_object_changes,
                ..
            } => {
                // Use native effects if available, otherwise convert RPC object changes
                if let Some(native) = native_effects {
                    native.object_changes()
                } else if !rpc_object_changes.is_empty() {
                    rpc_object_changes
                        .iter()
                        .map(Self::rpc_to_native_object_change)
                        .collect()
                } else {
                    return Ok(connection);
                }
            }
        };

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
            TransactionBlockEffectsKind::DryRun {
                balance_changes, ..
            } => balance_changes.len(),
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
                TransactionBlockEffectsKind::DryRun {
                    balance_changes, ..
                } => balance_changes.get(c.ix).cloned(),
            };

            let Some(serialized) = serialized else {
                continue;
            };

            let balance_change = BalanceChange::read(serialized.as_slice(), c.c).extend()?;
            connection
                .edges
                .push(Edge::new(c.encode_cursor(), balance_change));
        }

        Ok(connection)
    }

    /// Events emitted by this transaction block.
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
                    stored: None,
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
    async fn timestamp(&self) -> Result<Option<DateTime>, Error> {
        let TransactionBlockEffectsKind::Checkpointed { stored_tx, .. } = &self.kind else {
            return Ok(None);
        };
        Ok(Some(DateTime::from_ms(stored_tx.timestamp_ms)?))
    }

    /// The epoch this transaction was finalized in.
    async fn epoch(&self, ctx: &Context<'_>) -> Result<Option<Epoch>> {
        let rpc = self.rpc();
        Epoch::query(ctx, Some(rpc.executed_epoch()), self.checkpoint_viewed_at)
            .await
            .extend()
    }

    /// The checkpoint this transaction was finalized in.
    async fn checkpoint(&self, ctx: &Context<'_>) -> Result<Option<Checkpoint>> {
        // If the transaction data is not a checkpointed transaction, it's not in the
        // checkpoint yet so we return None.
        let TransactionBlockEffectsKind::Checkpointed { stored_tx, .. } = &self.kind else {
            return Ok(None);
        };

        Checkpoint::query(
            ctx,
            CheckpointId::by_seq_num(stored_tx.checkpoint_sequence_number as u64),
            self.checkpoint_viewed_at,
        )
        .await
        .extend()
    }

    /// Base64 encoded bcs serialization of the on-chain transaction effects.
    async fn bcs(&self) -> Result<Option<Base64>> {
        let bytes = match &self.kind {
            TransactionBlockEffectsKind::Checkpointed { stored_tx, .. } => {
                Some(stored_tx.raw_effects.clone())
            }
            TransactionBlockEffectsKind::Executed { optimistic_tx, .. } => {
                Some(optimistic_tx.raw_effects.clone())
            }
            TransactionBlockEffectsKind::DryRun { native_effects, .. } => {
                // Only return BCS data if native effects are available
                match native_effects {
                    Some(native) => Some(
                        bcs::to_bytes(native)
                            .map_err(|e| {
                                Error::Internal(format!(
                                    "Error serializing transaction effects: {e}"
                                ))
                            })
                            .extend()?,
                    ),
                    None => None,
                }
            }
        };

        Ok(bytes.map(Base64::from))
    }
}

impl TransactionBlockEffects {
    fn rpc(&self) -> &IotaTransactionBlockEffects {
        match &self.kind {
            TransactionBlockEffectsKind::Checkpointed { rpc, .. } => rpc,
            TransactionBlockEffectsKind::DryRun { rpc, .. } => rpc,
            TransactionBlockEffectsKind::Executed { rpc, .. } => rpc,
        }
    }

    fn native(&self) -> Option<&NativeTransactionEffects> {
        match &self.kind {
            TransactionBlockEffectsKind::Checkpointed { native, .. } => Some(native),
            TransactionBlockEffectsKind::DryRun { native_effects, .. } => native_effects.as_ref(),
            TransactionBlockEffectsKind::Executed { native, .. } => Some(native),
        }
    }

    /// Get the transaction data from the transaction block effects.
    /// Will error if the transaction data is not available/invalid, but this
    /// should not occur.
    fn transaction_data(&self) -> Result<NativeTransactionData> {
        Ok(match &self.kind {
            TransactionBlockEffectsKind::Checkpointed { stored_tx, .. } => {
                let s: NativeSenderSignedData = bcs::from_bytes(&stored_tx.raw_transaction)
                    .map_err(|e| {
                        Error::Internal(format!("Error deserializing transaction data: {e}"))
                    })?;
                s.transaction_data().clone()
            }
            TransactionBlockEffectsKind::DryRun { tx_data, .. } => tx_data.clone(),
            TransactionBlockEffectsKind::Executed { optimistic_tx, .. } => {
                let data: NativeSenderSignedData = bcs::from_bytes(&optimistic_tx.raw_transaction)
                    .map_err(|e| {
                        Error::Internal(format!("Error deserializing transaction data: {e}"))
                    })?;
                data.transaction_data().clone()
            }
        })
    }

    /// Get the programmable transaction from the transaction block effects.
    /// * If the transaction was unable to be retrieved, this will return an
    ///   Err.
    /// * If the transaction was able to be retrieved but was not a programmable
    ///   transaction, this will return Ok(None).
    /// * If the transaction was a programmable transaction, this will return
    ///   Ok(Some(tx)).
    /// Convert RPC ObjectChange to native ObjectChange format
    fn rpc_to_native_object_change(rpc_change: &RpcObjectChange) -> NativeObjectChange {
        use iota_types::effects::IDOperation;

        match rpc_change {
            RpcObjectChange::Published { package_id, .. } => NativeObjectChange {
                id: *package_id,
                input_version: None,
                input_digest: None,
                output_version: Some(1u64.into()),
                output_digest: None,
                id_operation: IDOperation::Created,
            },
            RpcObjectChange::Transferred {
                object_id, version, ..
            } => NativeObjectChange {
                id: *object_id,
                input_version: None,
                input_digest: None,
                output_version: Some(*version),
                output_digest: None,
                id_operation: IDOperation::None,
            },
            RpcObjectChange::Mutated {
                object_id,
                version,
                previous_version,
                ..
            } => NativeObjectChange {
                id: *object_id,
                input_version: Some(*previous_version),
                input_digest: None,
                output_version: Some(*version),
                output_digest: None,
                id_operation: IDOperation::None,
            },
            RpcObjectChange::Deleted {
                object_id, version, ..
            } => NativeObjectChange {
                id: *object_id,
                input_version: Some(*version),
                input_digest: None,
                output_version: None,
                output_digest: None,
                id_operation: IDOperation::Deleted,
            },
            RpcObjectChange::Wrapped {
                object_id, version, ..
            } => NativeObjectChange {
                id: *object_id,
                input_version: Some(*version),
                input_digest: None,
                output_version: None,
                output_digest: None,
                id_operation: IDOperation::Deleted,
            },
            RpcObjectChange::Created {
                object_id, version, ..
            } => NativeObjectChange {
                id: *object_id,
                input_version: None,
                input_digest: None,
                output_version: Some(*version),
                output_digest: None,
                id_operation: IDOperation::Created,
            },
        }
    }

    fn programmable_transaction(&self) -> Result<Option<ProgrammableTransaction>> {
        let tx_data = self.transaction_data()?;
        match tx_data.into_kind() {
            NativeTransactionKind::ProgrammableTransaction(tx) => Ok(Some(tx)),
            _ => Ok(None),
        }
    }

    /// Resolves the module ID within a Move abort to the storage ID of the
    /// package that the abort occurred in.
    /// * If the error is not a Move abort, or the Move call in the programmable
    ///   transaction cannot be found, this function will do nothing.
    /// * If the error is a Move abort and the storage ID is unable to be
    ///   resolved an error is returned.
    async fn resolve_native_status_impl(
        &self,
        resolver: &PackageResolver,
    ) -> Result<NativeExecutionStatus> {
        let mut status = self.native().as_ref().unwrap().status().clone();
        if let NativeExecutionStatus::Failure {
            error:
                ExecutionFailureStatus::MoveAbort(MoveLocation { module, .. }, _)
                | ExecutionFailureStatus::MovePrimitiveRuntimeError(MoveLocationOpt(Some(MoveLocation {
                    module,
                    ..
                }))),
            command: Some(command_idx),
        } = &mut status
        {
            // Get the Move call that this error is associated with.
            if let Some(Command::MoveCall(ptb_call)) = self
                .programmable_transaction()?
                .and_then(|ptb| ptb.commands.into_iter().nth(*command_idx))
            {
                let module_new = module.clone();
                // Resolve the runtime module ID in the Move abort to the storage ID of the
                // package that the abort occurred in. This is important to make
                // sure that we look at the correct version of the module when
                // resolving the error.
                *module = resolver
                    .resolve_module_id(module_new, ptb_call.package.into())
                    .await
                    .map_err(|e| Error::Internal(format!("Error resolving Move location: {e}")))?;
            }
        }
        Ok(status)
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
        let native: NativeTransactionEffects = bcs::from_bytes(&optimistic_tx.raw_effects).map_err(|e| {
            Error::Internal(format!(
                "Failed to deserialize NativeTransactionEffects from optimistic transaction: {e}"
            ))
        })?;

        let rpc: IotaTransactionBlockEffects = native.clone().try_into().map_err(|e| {
            Error::Internal(format!(
                "Failed to convert native effects to RPC effects: {e}"
            ))
        })?;

        Ok(TransactionBlockEffectsKind::Executed {
            optimistic_tx,
            native,
            rpc,
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
                let native: NativeTransactionEffects = bcs::from_bytes(&stored_tx.raw_effects)
                    .map_err(|e| {
                        Error::Internal(format!("Error deserializing transaction effects: {e}"))
                    })?;

                let rpc: IotaTransactionBlockEffects = native.clone().try_into().map_err(|e| {
                    Error::Internal(format!(
                        "Failed to convert native effects to RPC effects: {e}"
                    ))
                })?;

                Ok(TransactionBlockEffectsKind::Checkpointed {
                    stored_tx: stored_tx.clone(),
                    native,
                    rpc,
                })
            }
            TransactionBlockInner::Executed { optimistic_tx, .. } => {
                TransactionBlockEffectsKind::try_from(optimistic_tx.clone())
            }

            TransactionBlockInner::DryRun {
                tx_data,
                native_effects,
                rpc,
                rpc_object_changes,
                events,
                balance_changes,
            } => Ok(TransactionBlockEffectsKind::DryRun {
                tx_data,
                native_effects,
                rpc,
                rpc_object_changes,
                events,
                balance_changes,
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
