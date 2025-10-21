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
    BalanceChange, IotaTransactionBlock, IotaTransactionBlockEffectsAPI,
    IotaTransactionBlockEvents, IotaTransactionBlockResponse, ObjectChange,
};
use iota_metrics::spawn_monitored_task;
use iota_types::{
    base_types::{IotaAddress, ObjectDigest, ObjectID, SequenceNumber, TransactionDigest},
    crypto::default_hash,
    effects::{TransactionEffects, TransactionEffectsAPI, TransactionEvents},
    object::Owner as CoreOwner,
    quorum_driver_types::{
        ExecuteTransactionRequestType, ExecuteTransactionRequestV1, ExecuteTransactionResponseV1,
        IsTransactionExecutedLocally,
    },
    signature::GenericSignature,
    storage::PostExecutionPackageResolver,
    transaction::{
        InputObjectKind, Transaction, TransactionData, TransactionDataAPI, TransactionKind,
    },
};
use shared_crypto::intent::{AppId, Intent, IntentMessage, IntentScope, IntentVersion};
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
        opts: Option<grpc_common::TransactionResponseOptions>,
        epoch_store: &Arc<AuthorityPerEpochStore>,
    ) -> Result<
        (
            ExecuteTransactionRequestV1,
            grpc_common::TransactionResponseOptions,
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

    /// Prepare dry run transaction data
    /// Extracts transaction data, calculates digest, and gets input objects
    fn prepare_dry_run_transaction(
        &self,
        tx_bytes: Vec<u8>,
    ) -> Result<(TransactionData, TransactionDigest, Vec<InputObjectKind>), Status> {
        let txn_data: TransactionData = bcs::from_bytes(&tx_bytes)
            .map_err(|e| Status::invalid_argument(format!("Invalid transaction data: {e}")))?;

        let input_objs = txn_data
            .input_objects()
            .map_err(|e| Status::invalid_argument(format!("Failed to get input objects: {e}")))?;

        let intent_msg = IntentMessage::new(
            Intent {
                version: IntentVersion::V0,
                scope: IntentScope::TransactionData,
                app_id: AppId::Iota,
            },
            txn_data.clone(),
        );
        let txn_digest = TransactionDigest::new(default_hash(&intent_msg.value));

        Ok((txn_data, txn_digest, input_objs))
    }

    /// Convert IotaTransactionBlockResponse to proto ExecuteTransactionResponse
    /// Uses core BCS types for transaction/effects/events and native proto
    /// messages for ObjectChange/BalanceChange
    fn convert_response_to_proto(
        response: &IotaTransactionBlockResponse,
        core_transaction_data: Option<&TransactionData>,
        core_effects: Option<&TransactionEffects>,
        core_events: Option<&TransactionEvents>,
    ) -> Result<grpc_common::TransactionResponse, Status> {
        Ok(grpc_common::TransactionResponse {
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
    ) -> Result<grpc_common::ObjectChange, Status> {
        let change_oneof = match change {
            ObjectChange::Published {
                package_id,
                version,
                digest,
                modules,
            } => grpc_common::object_change::Change::Published(grpc_common::ObjectPublished {
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
            } => grpc_common::object_change::Change::Transferred(grpc_common::ObjectTransferred {
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
            } => grpc_common::object_change::Change::Mutated(grpc_common::ObjectMutated {
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
            } => grpc_common::object_change::Change::Deleted(grpc_common::ObjectDeleted {
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
            } => grpc_common::object_change::Change::Wrapped(grpc_common::ObjectWrapped {
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
            } => grpc_common::object_change::Change::Created(grpc_common::ObjectCreated {
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

        Ok(grpc_common::ObjectChange {
            change: Some(change_oneof),
        })
    }

    /// Convert BalanceChange (JSON-RPC type) to proto BalanceChange
    fn convert_balance_change_to_proto(
        change: &BalanceChange,
    ) -> Result<grpc_common::BalanceChange, Status> {
        Ok(grpc_common::BalanceChange {
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
        opts: grpc_common::TransactionResponseOptions,
        digest: TransactionDigest,
        input_objs: Vec<InputObjectKind>,
        transaction: Option<IotaTransactionBlock>,
        raw_transaction: Vec<u8>,
        sender: IotaAddress,
        core_transaction_data: Option<TransactionData>,
        epoch_store: &Arc<AuthorityPerEpochStore>,
    ) -> Result<Response<grpc_common::TransactionResponse>, Status> {
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
    ) -> Result<Response<grpc_common::TransactionResponse>, Status> {
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

        // Extract bytes from BcsData wrapper
        let tx_bytes = req
            .tx_bytes
            .ok_or_else(|| Status::invalid_argument("tx_bytes is required"))?
            .data;
        let signatures = req.signatures.into_iter().map(|s| s.data).collect();

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
            tx_bytes,
            signatures,
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

    #[instrument("grpc_api_dry_run_transaction", level = "trace", skip_all)]
    async fn dry_run_transaction(
        &self,
        request: Request<grpc_write::DryRunTransactionRequest>,
    ) -> Result<Response<grpc_write::DryRunTransactionResponse>, Status> {
        let req = request.into_inner();

        // Extract tx_bytes from BcsData
        let tx_bytes = req
            .tx_bytes
            .ok_or_else(|| Status::invalid_argument("tx_bytes is required"))?
            .data;

        // Prepare transaction data using helper method
        let (txn_data, txn_digest, input_objs) = self.prepare_dry_run_transaction(tx_bytes)?;
        let sender = txn_data.sender();

        // Get authority state
        let authority_state = self
            .grpc_reader
            .authority_state()
            .ok_or_else(|| Status::internal("Authority state not available"))?;

        // Use spawn_blocking since dry_exec_transaction is a long-running synchronous
        // operation
        let state = authority_state.clone();
        let txn_data_clone = txn_data.clone();

        // First await the spawn_blocking task then unwrap both Results
        let join_handle = spawn_monitored_task!(tokio::task::spawn_blocking(move || {
            state.dry_exec_transaction(txn_data_clone, txn_digest)
        }));

        let join_result = join_handle
            .await
            .map_err(|e| Status::internal(format!("Task join failed: {e}")))?;
        let (resp, written_objects, transaction_effects, mock_gas) =
            join_result.map_err(|e| Status::internal(format!("Dry run failed: {e}")))??;

        // Calculate object changes and balance changes
        let object_cache =
            ObjectProviderCache::new_with_cache(authority_state.clone(), written_objects);

        let balance_changes = get_balance_changes_from_effect(
            &object_cache,
            &transaction_effects,
            input_objs,
            mock_gas,
        )
        .instrument(tracing::trace_span!("resolving balance changes"))
        .await
        .map_err(|e| Status::internal(format!("Failed to get balance changes: {e}")))?;

        let object_changes = get_object_changes(
            &object_cache,
            sender,
            transaction_effects.modified_at_versions(),
            transaction_effects.all_changed_objects(),
            transaction_effects.all_removed_objects(),
        )
        .instrument(tracing::trace_span!("resolving object changes"))
        .await
        .map_err(|e| Status::internal(format!("Failed to get object changes: {e}")))?;

        // Serialize effects using core TransactionEffects (iota-types), not JSON-RPC
        // type
        let effects_bcs = bcs::to_bytes(&transaction_effects)
            .map(|data| grpc_common::BcsData { data })
            .map_err(|e| Status::internal(format!("Failed to serialize effects: {e}")))?;

        // Convert object_changes and balance_changes to proto
        let proto_object_changes = object_changes
            .iter()
            .map(crate::read_service::ReadGrpcService::convert_object_change_to_proto)
            .collect::<Result<Vec<_>, _>>()?;

        let proto_balance_changes = balance_changes
            .iter()
            .map(crate::read_service::ReadGrpcService::convert_balance_change_to_proto)
            .collect::<Result<Vec<_>, _>>()?;

        // Extract execution_error_source from the transaction effects status
        // Convert to JSON-RPC type which has public fields
        let iota_effects: iota_json_rpc_types::IotaTransactionBlockEffects = transaction_effects
            .clone()
            .try_into()
            .map_err(|e| Status::internal(format!("Failed to convert effects: {e}")))?;

        let execution_error_source = match iota_effects.status() {
            iota_json_rpc_types::IotaExecutionStatus::Failure { error } => Some(error.clone()),
            iota_json_rpc_types::IotaExecutionStatus::Success => None,
        };

        // Convert events from IotaTransactionBlockEvents to proto Event messages
        let events_proto: Vec<grpc_common::Event> = resp
            .events
            .data
            .iter()
            .map(grpc_common::Event::from)
            .collect();

        // Convert core TransactionData to proto TransactionData
        // Similar to JSON-RPC API, we provide both:
        // 1. Full BCS-encoded TransactionData for complete information
        // 2. Extracted gas_data field for client convenience (avoids BCS
        //    deserialization)
        let proto_input = grpc_common::TransactionData {
            transaction: Some(grpc_common::BcsData {
                data: bcs::to_bytes(&txn_data).map_err(|e| {
                    Status::internal(format!("Failed to serialize transaction data: {e}"))
                })?,
            }),
            sender: Some(grpc_common::Address {
                address: sender.to_vec(),
            }),
            // Extract gas data from TransactionData for client convenience
            // This matches the JSON-RPC API pattern where gas data is provided separately
            gas_data: Some(grpc_common::GasData {
                payment: txn_data
                    .gas()
                    .iter()
                    .map(|obj_ref| grpc_common::ObjectRef {
                        object_id: Some(grpc_common::Address {
                            address: obj_ref.0.into_bytes().to_vec(),
                        }),
                        version: obj_ref.1.value(),
                        digest: Some(grpc_common::Digest {
                            digest: obj_ref.2.into_inner().to_vec(),
                        }),
                    })
                    .collect(),
                owner: Some(grpc_common::Address {
                    address: txn_data.gas_owner().to_vec(),
                }),
                price: txn_data.gas_price(),
                budget: txn_data.gas_budget(),
            }),
        };

        let response = grpc_write::DryRunTransactionResponse {
            effects: Some(effects_bcs),
            events: events_proto,
            object_changes: proto_object_changes,
            balance_changes: proto_balance_changes,
            input: Some(proto_input),
            execution_error_source,
            suggested_gas_price: resp.suggested_gas_price,
        };

        debug!("Dry run transaction completed successfully");
        Ok(Response::new(response))
    }

    #[instrument("grpc_api_dev_inspect_transaction", level = "trace", skip_all)]
    async fn dev_inspect_transaction(
        &self,
        request: Request<grpc_write::DevInspectTransactionRequest>,
    ) -> Result<Response<grpc_write::DevInspectTransactionResponse>, Status> {
        let req = request.into_inner();

        // Extract sender address
        let sender_bytes = req
            .sender
            .ok_or_else(|| Status::invalid_argument("sender is required"))?
            .address;
        let sender = IotaAddress::from_bytes(&sender_bytes)
            .map_err(|e| Status::invalid_argument(format!("Invalid sender address: {e}")))?;

        // Extract tx_bytes from BcsData
        let tx_bytes = req
            .tx_bytes
            .ok_or_else(|| Status::invalid_argument("tx_bytes is required"))?
            .data;

        // Deserialize TransactionKind
        let tx_kind: TransactionKind = bcs::from_bytes(&tx_bytes)
            .map_err(|e| Status::invalid_argument(format!("Invalid transaction kind: {e}")))?;

        // Extract optional parameters
        let gas_price = req.gas_price;
        let additional_args = req.additional_args.unwrap_or_default();

        // Get authority state
        let authority_state = self
            .grpc_reader
            .authority_state()
            .ok_or_else(|| Status::internal("Authority state not available"))?;

        // Call dev_inspect_transaction_block
        // We need raw_effects for the gRPC response, but respect show_txn_data for
        // raw_txn_data Since the authority API uses a single flag for both, we
        // pass true to always get raw_effects
        let results = authority_state
            .dev_inspect_transaction_block(
                sender,
                tx_kind,
                gas_price,
                additional_args.gas_budget,
                additional_args
                    .gas_sponsor
                    .and_then(|addr| IotaAddress::from_bytes(&addr.address).ok()),
                if additional_args.gas_objects.is_empty() {
                    None
                } else {
                    Some(
                        additional_args
                            .gas_objects
                            .into_iter()
                            .filter_map(|obj_ref| {
                                let object_id =
                                    ObjectID::from_bytes(&obj_ref.object_id?.address).ok()?;
                                let version = SequenceNumber::from_u64(obj_ref.version);
                                let digest =
                                    ObjectDigest::try_from(obj_ref.digest?.digest.as_slice())
                                        .ok()?;
                                Some((object_id, version, digest))
                            })
                            .collect::<Vec<_>>(),
                    )
                },
                Some(true), // Always get raw effects and raw txn data
                additional_args.skip_checks,
            )
            .await
            .map_err(|e| Status::internal(format!("Dev inspect failed: {e}")))?;

        // Use raw_effects which contains BCS-encoded iota-types TransactionEffects
        // This matches the protobuf definition which specifies TransactionEffects (not
        // IotaTransactionBlockEffects)
        let effects_bcs = grpc_common::BcsData {
            data: results.raw_effects,
        };

        // Convert events from IotaTransactionBlockEvents to proto Event messages
        let events_proto: Vec<grpc_common::Event> = results
            .events
            .data
            .iter()
            .map(grpc_common::Event::from)
            .collect();

        // Serialize results (execution results) - repeated BcsData
        let results_bcs = results
            .results
            .unwrap_or_default()
            .iter()
            .map(|r| {
                bcs::to_bytes(r)
                    .map(|data| grpc_common::BcsData { data })
                    .map_err(|e| Status::internal(format!("Failed to serialize results: {e}")))
            })
            .collect::<Result<Vec<_>, _>>()?;

        // Return txn_data only if show_txn_data is requested
        let txn_data =
            if additional_args.show_txn_data.unwrap_or(false) && !results.raw_txn_data.is_empty() {
                Some(grpc_common::BcsData {
                    data: results.raw_txn_data,
                })
            } else {
                None
            };

        let response = grpc_write::DevInspectTransactionResponse {
            effects: Some(effects_bcs),
            events: events_proto,
            results: results_bcs,
            error: results.error,
            txn_data,
        };

        debug!("Dev inspect transaction completed successfully");
        Ok(Response::new(response))
    }
}
