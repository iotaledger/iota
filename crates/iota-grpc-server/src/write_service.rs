// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use fastcrypto::traits::ToFromBytes;
use iota_core::{
    authority::authority_per_epoch_store::AuthorityPerEpochStore,
    authority_client::NetworkAuthorityClient, transaction_orchestrator::TransactionOrchestrator,
};
use iota_grpc_types::v0::{common as grpc_common, write as grpc_write};
use iota_json_rpc::{ObjectProviderCache, get_balance_changes_from_effect, get_object_changes};
use iota_json_rpc_types::{
    BalanceChange, IotaTransactionBlock, IotaTransactionBlockEvents, IotaTransactionBlockResponse,
    ObjectChange,
};
use iota_metrics::spawn_monitored_task;
use iota_types::{
    base_types::{IotaAddress, TransactionDigest},
    effects::{TransactionEffects, TransactionEffectsAPI, TransactionEvents},
    object::Owner as CoreOwner,
    quorum_driver_types::{
        ExecuteTransactionRequestType, ExecuteTransactionRequestV1, ExecuteTransactionResponseV1,
        IsTransactionExecutedLocally,
    },
    signature::GenericSignature,
    storage::PostExecutionPackageResolver,
    transaction::{InputObjectKind, Transaction, TransactionData, TransactionDataAPI},
};
use tonic::{Request, Response, Status};
use tracing::{Instrument, debug, instrument};

use crate::GrpcReader;

pub struct WriteGrpcService {
    /// Transaction orchestrator
    pub transaction_orchestrator: Option<Arc<TransactionOrchestrator<NetworkAuthorityClient>>>,
    /// GrpcReader for data access including epoch store when available
    pub grpc_reader: Arc<GrpcReader>,
}

impl WriteGrpcService {
    pub fn new(
        transaction_orchestrator: Option<Arc<TransactionOrchestrator<NetworkAuthorityClient>>>,
        grpc_reader: Arc<GrpcReader>,
    ) -> Self {
        Self {
            transaction_orchestrator,
            grpc_reader,
        }
    }

    /// Prepare transaction request
    #[expect(clippy::type_complexity)]
    fn prepare_execute_transaction_request(
        &self,
        tx_bytes: Vec<u8>,
        signatures: Vec<Vec<u8>>,
        opts: Option<grpc_write::TransactionResponseOptions>,
        epoch_store: &Arc<AuthorityPerEpochStore>,
    ) -> Result<
        (
            ExecuteTransactionRequestV1,
            grpc_write::TransactionResponseOptions,
            IotaAddress,
            Vec<InputObjectKind>,
            Transaction,
            Option<IotaTransactionBlock>,
            Vec<u8>,
            TransactionData, // Added: return TransactionData for BCS serialization
        ),
        Status,
    > {
        let opts = opts.unwrap_or_default();
        let tx_data: TransactionData = bcs::from_bytes(&tx_bytes)
            .map_err(|e| Status::invalid_argument(format!("Failed to deserialize: {e}")))?;
        let sender = tx_data.sender();
        let input_objs = tx_data.input_objects().unwrap_or_default();

        // Clone tx_data before consuming it
        let tx_data_clone = tx_data.clone();

        let mut sigs = Vec::new();
        for sig in signatures {
            let signature = GenericSignature::from_bytes(&sig)
                .map_err(|e| Status::invalid_argument(format!("Invalid signature: {e}")))?;
            sigs.push(signature);
        }
        let txn = Transaction::from_generic_sig_data(tx_data, sigs);
        let raw_transaction = if opts.show_raw_input {
            bcs::to_bytes(txn.data()).map_err(|e| {
                Status::internal(format!("Raw transaction serialization failed: {e}"))
            })?
        } else {
            vec![]
        };

        let transaction_block = if opts.show_input {
            Some(
                IotaTransactionBlock::try_from(
                    txn.data().clone(),
                    epoch_store.module_cache(),
                    *txn.digest(),
                )
                .map_err(|e| {
                    Status::internal(format!("Failed to create IotaTransactionBlock: {e}"))
                })?,
            )
        } else {
            None
        };

        let request = ExecuteTransactionRequestV1 {
            transaction: txn.clone(),
            include_events: opts.show_events,
            include_input_objects: opts.show_balance_changes || opts.show_object_changes,
            include_output_objects: opts.show_balance_changes
                || opts.show_object_changes
                || opts.show_events,
            include_auxiliary_data: false,
        };

        Ok((
            request,
            opts,
            sender,
            input_objs,
            txn,
            transaction_block,
            raw_transaction,
            tx_data_clone, // Return cloned TransactionData
        ))
    }

    /// Convert IotaTransactionBlockResponse to proto ExecuteTransactionResponse
    /// Uses core BCS types for transaction/effects/events and native proto
    /// messages for ObjectChange/BalanceChange
    fn convert_response_to_proto(
        response: &IotaTransactionBlockResponse,
        core_transaction_data: Option<&TransactionData>,
        core_effects: Option<&TransactionEffects>,
        core_events: Option<&TransactionEvents>,
    ) -> Result<grpc_write::ExecuteTransactionResponse, Status> {
        Ok(grpc_write::ExecuteTransactionResponse {
            digest: Some(grpc_common::Digest {
                digest: response.digest.into_inner().to_vec(),
            }),
            // Use core TransactionData (BCS-serializable)
            transaction: core_transaction_data
                .map(|t| {
                    bcs::to_bytes(t)
                        .map(|data| grpc_common::BcsData { data })
                        .map_err(|e| {
                            Status::internal(format!("Failed to serialize TransactionData: {e}"))
                        })
                })
                .transpose()?,
            raw_transaction: if response.raw_transaction.is_empty() {
                None
            } else {
                Some(grpc_common::BcsData {
                    data: response.raw_transaction.clone(),
                })
            },
            // Use core TransactionEffects (BCS-serializable)
            effects: core_effects
                .map(|e| {
                    bcs::to_bytes(e)
                        .map(|data| grpc_common::BcsData { data })
                        .map_err(|e| {
                            Status::internal(format!("Failed to serialize TransactionEffects: {e}"))
                        })
                })
                .transpose()?,
            // Use core TransactionEvents (BCS-serializable)
            events: core_events
                .map(|e| {
                    bcs::to_bytes(e)
                        .map(|data| grpc_common::BcsData { data })
                        .map_err(|e| {
                            Status::internal(format!("Failed to serialize TransactionEvents: {e}"))
                        })
                })
                .transpose()?,
            // Convert ObjectChange to native proto messages
            object_changes: response
                .object_changes
                .as_ref()
                .map(|changes| {
                    changes
                        .iter()
                        .map(Self::convert_object_change_to_proto)
                        .collect::<Result<Vec<_>, _>>()
                })
                .transpose()?
                .unwrap_or_default(),
            // Convert BalanceChange to native proto messages
            balance_changes: response
                .balance_changes
                .as_ref()
                .map(|changes| {
                    changes
                        .iter()
                        .map(Self::convert_balance_change_to_proto)
                        .collect::<Result<Vec<_>, _>>()
                })
                .transpose()?
                .unwrap_or_default(),
            timestamp_ms: response.timestamp_ms,
            confirmed_local_execution: response.confirmed_local_execution,
            checkpoint: response.checkpoint,
            errors: response.errors.clone(),
            raw_effects: if response.raw_effects.is_empty() {
                None
            } else {
                Some(grpc_common::BcsData {
                    data: response.raw_effects.clone(),
                })
            },
        })
    }

    /// Convert ObjectChange (JSON-RPC type) to proto ObjectChange
    fn convert_object_change_to_proto(
        change: &ObjectChange,
    ) -> Result<grpc_write::ObjectChange, Status> {
        let change_oneof = match change {
            ObjectChange::Published {
                package_id,
                version,
                digest,
                modules,
            } => grpc_write::object_change::Change::Published(grpc_write::ObjectPublished {
                package_id: Some(grpc_common::Address {
                    address: package_id.into_bytes().to_vec(),
                }),
                version: version.value(),
                digest: Some(grpc_common::Digest {
                    digest: digest.into_inner().to_vec(),
                }),
                modules: modules.clone(),
            }),
            ObjectChange::Transferred {
                sender,
                recipient,
                object_type,
                object_id,
                version,
                digest,
            } => grpc_write::object_change::Change::Transferred(grpc_write::ObjectTransferred {
                sender: Some(grpc_common::Address {
                    address: sender.to_vec(),
                }),
                recipient: Some(Self::convert_owner_to_proto(recipient)?),
                object_type: object_type.to_string(),
                object_id: Some(grpc_common::Address {
                    address: object_id.into_bytes().to_vec(),
                }),
                version: version.value(),
                digest: Some(grpc_common::Digest {
                    digest: digest.into_inner().to_vec(),
                }),
            }),
            ObjectChange::Mutated {
                sender,
                owner,
                object_type,
                object_id,
                version,
                previous_version,
                digest,
            } => grpc_write::object_change::Change::Mutated(grpc_write::ObjectMutated {
                sender: Some(grpc_common::Address {
                    address: sender.to_vec(),
                }),
                owner: Some(Self::convert_owner_to_proto(owner)?),
                object_type: object_type.to_string(),
                object_id: Some(grpc_common::Address {
                    address: object_id.into_bytes().to_vec(),
                }),
                version: version.value(),
                previous_version: previous_version.value(),
                digest: Some(grpc_common::Digest {
                    digest: digest.into_inner().to_vec(),
                }),
            }),
            ObjectChange::Deleted {
                sender,
                object_type,
                object_id,
                version,
            } => grpc_write::object_change::Change::Deleted(grpc_write::ObjectDeleted {
                sender: Some(grpc_common::Address {
                    address: sender.to_vec(),
                }),
                object_type: object_type.to_string(),
                object_id: Some(grpc_common::Address {
                    address: object_id.into_bytes().to_vec(),
                }),
                version: version.value(),
            }),
            ObjectChange::Wrapped {
                sender,
                object_type,
                object_id,
                version,
            } => grpc_write::object_change::Change::Wrapped(grpc_write::ObjectWrapped {
                sender: Some(grpc_common::Address {
                    address: sender.to_vec(),
                }),
                object_type: object_type.to_string(),
                object_id: Some(grpc_common::Address {
                    address: object_id.into_bytes().to_vec(),
                }),
                version: version.value(),
            }),
            ObjectChange::Created {
                sender,
                owner,
                object_type,
                object_id,
                version,
                digest,
            } => grpc_write::object_change::Change::Created(grpc_write::ObjectCreated {
                sender: Some(grpc_common::Address {
                    address: sender.to_vec(),
                }),
                owner: Some(Self::convert_owner_to_proto(owner)?),
                object_type: object_type.to_string(),
                object_id: Some(grpc_common::Address {
                    address: object_id.into_bytes().to_vec(),
                }),
                version: version.value(),
                digest: Some(grpc_common::Digest {
                    digest: digest.into_inner().to_vec(),
                }),
            }),
        };

        Ok(grpc_write::ObjectChange {
            change: Some(change_oneof),
        })
    }

    /// Convert BalanceChange (JSON-RPC type) to proto BalanceChange
    fn convert_balance_change_to_proto(
        change: &BalanceChange,
    ) -> Result<grpc_write::BalanceChange, Status> {
        Ok(grpc_write::BalanceChange {
            owner: Some(Self::convert_owner_to_proto(&change.owner)?),
            coin_type: change.coin_type.to_string(),
            amount: change.amount.to_string(),
        })
    }

    /// Convert Owner to proto Owner
    fn convert_owner_to_proto(owner: &CoreOwner) -> Result<grpc_common::Owner, Status> {
        let owner_oneof = match owner {
            CoreOwner::AddressOwner(addr) => {
                grpc_common::owner::Owner::AddressOwner(grpc_common::Address {
                    address: addr.to_vec(),
                })
            }
            CoreOwner::ObjectOwner(addr) => {
                grpc_common::owner::Owner::ObjectOwner(grpc_common::Address {
                    address: addr.to_vec(),
                })
            }
            CoreOwner::Shared {
                initial_shared_version,
            } => grpc_common::owner::Owner::Shared(grpc_common::SharedOwner {
                initial_shared_version: initial_shared_version.value(),
            }),
            CoreOwner::Immutable => {
                grpc_common::owner::Owner::Immutable(grpc_common::ImmutableOwner {})
            }
        };

        Ok(grpc_common::Owner {
            owner: Some(owner_oneof),
        })
    }

    async fn handle_post_orchestration(
        &self,
        response: ExecuteTransactionResponseV1,
        is_executed_locally: IsTransactionExecutedLocally,
        opts: grpc_write::TransactionResponseOptions,
        digest: TransactionDigest,
        input_objs: Vec<InputObjectKind>,
        transaction: Option<IotaTransactionBlock>,
        raw_transaction: Vec<u8>,
        sender: IotaAddress,
        core_transaction_data: Option<TransactionData>,
        epoch_store: &Arc<AuthorityPerEpochStore>,
    ) -> Result<Response<grpc_write::ExecuteTransactionResponse>, Status> {
        // Store core events for BCS serialization before converting to JSON type
        let core_events = response.events.clone();

        let events = if opts.show_events {
            tracing::trace!("Resolving events");
            if let Some(authority_state) = self.grpc_reader.authority_state().as_ref() {
                let backing_package_store = PostExecutionPackageResolver::new(
                    authority_state.get_backing_package_store().clone(),
                    &response.output_objects,
                );
                let mut layout_resolver = epoch_store
                    .executor()
                    .type_layout_resolver(Box::new(backing_package_store));
                Some(
                    IotaTransactionBlockEvents::try_from(
                        core_events.clone().unwrap_or_default(),
                        digest,
                        None,
                        layout_resolver.as_mut(),
                    )
                    .map_err(|e| Status::internal(format!("Failed to convert events: {e}")))?,
                )
            } else {
                return Err(Status::internal(
                    "Cannot convert events: missing authority state",
                ));
            }
        } else {
            None
        };

        let object_cache = {
            response.output_objects.and_then(|output_objects| {
                self.grpc_reader.authority_state().map(|authority_state| {
                    ObjectProviderCache::new_with_output_objects(
                        authority_state.clone(),
                        output_objects,
                    )
                })
            })
        };

        let balance_changes = match &object_cache {
            Some(object_cache) if opts.show_balance_changes => Some(
                get_balance_changes_from_effect(
                    object_cache,
                    &response.effects.effects,
                    input_objs,
                    None,
                )
                .instrument(tracing::trace_span!("resolving balance changes"))
                .await
                .map_err(|e| Status::internal(format!("Failed to get balance changes: {e}")))?,
            ),
            _ => None,
        };

        let object_changes = match &object_cache {
            Some(object_cache) if opts.show_object_changes => Some(
                get_object_changes(
                    object_cache,
                    sender,
                    response.effects.effects.modified_at_versions(),
                    response.effects.effects.all_changed_objects(),
                    response.effects.effects.all_removed_objects(),
                )
                .instrument(tracing::trace_span!("resolving object changes"))
                .await
                .map_err(|e| Status::internal(format!("Failed to get object changes: {e}")))?,
            ),
            _ => None,
        };

        let raw_effects = if opts.show_raw_effects {
            bcs::to_bytes(&response.effects.effects)
                .map_err(|e| Status::internal(format!("Raw effects serialization failed: {e}")))?
        } else {
            vec![]
        };

        // Store core effects for BCS serialization before converting
        let core_effects = response.effects.effects.clone();

        let iota_response = IotaTransactionBlockResponse {
            digest,
            transaction,
            raw_transaction,
            effects: opts
                .show_effects
                .then(|| {
                    core_effects
                        .clone()
                        .try_into()
                        .map_err(|e| Status::internal(format!("Failed to convert effects: {e}")))
                })
                .transpose()?,
            events,
            object_changes,
            balance_changes,
            timestamp_ms: None,
            confirmed_local_execution: Some(is_executed_locally),
            checkpoint: None,
            errors: vec![],
            raw_effects,
        };

        // Convert to proto using core types
        let grpc_response = Self::convert_response_to_proto(
            &iota_response,
            core_transaction_data.as_ref(),
            opts.show_effects.then_some(&core_effects),
            core_events.as_ref(),
        )?;

        debug!("Transaction executed successfully");
        Ok(Response::new(grpc_response))
    }
}

// The `WriteService` is the auto-generated trait from the protobuf definition.
// It's generated by tonic/protobuf and defines the interface that any gRPC
// write service must implement.
#[tonic::async_trait]
impl grpc_write::write_service_server::WriteService for WriteGrpcService {
    #[instrument("grpc_api_execute_transaction", level = "trace", skip_all)]
    async fn execute_transaction(
        &self,
        request: Request<grpc_write::ExecuteTransactionRequest>,
    ) -> Result<Response<grpc_write::ExecuteTransactionResponse>, Status> {
        let req = request.into_inner();

        // Load epoch store once at the beginning to ensure consistency throughout the
        // request
        let epoch_store = self
            .grpc_reader
            .load_epoch_store_one_call_per_task()
            .ok_or_else(|| Status::internal("Epoch store not available"))?;

        let request_type = req
            .request_type
            .map(|rt| match rt {
                0 => ExecuteTransactionRequestType::WaitForEffectsCert,
                1 => ExecuteTransactionRequestType::WaitForLocalExecution,
                _ => ExecuteTransactionRequestType::WaitForEffectsCert, // fallback to default
            })
            .unwrap_or(ExecuteTransactionRequestType::WaitForEffectsCert);

        let (
            execute_request,
            opts,
            sender,
            input_objs,
            txn,
            transaction_block,
            raw_transaction,
            tx_data,
        ) = self.prepare_execute_transaction_request(
            req.tx_bytes,
            req.signatures,
            req.options,
            &epoch_store,
        )?;

        let digest = *txn.digest();

        let orchestrator = self
            .transaction_orchestrator
            .clone()
            .ok_or_else(|| Status::unimplemented("Transaction execution not available"))?;

        tracing::trace!("Spawning transaction orchestrator task for transaction: {digest}",);
        let (response, is_executed_locally) = spawn_monitored_task!(
            orchestrator.execute_transaction_block(execute_request, request_type, None)
        )
        .await
        .map_err(|e| Status::internal(format!("Task execution failed: {e}")))?
        .map_err(|e| Status::internal(format!("Transaction execution failed: {e}")))?;

        // Keep core transaction data for BCS serialization
        let core_transaction_data = opts.show_input.then_some(tx_data);

        self.handle_post_orchestration(
            response,
            is_executed_locally,
            opts,
            digest,
            input_objs,
            transaction_block,
            raw_transaction,
            sender,
            core_transaction_data,
            &epoch_store,
        )
        .await
    }
}
