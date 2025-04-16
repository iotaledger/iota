// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeMap, sync::Arc};

use fastcrypto::encoding::Base64;
use futures::{StreamExt, stream};
use futures_core::Stream;
use iota_json_rpc_api::{
    GovernanceReadApiClient, IndexerApiClient, MoveUtilsClient, ReadApiClient, WriteApiClient,
};
use iota_json_rpc_types::{
    Checkpoint, CheckpointId, CheckpointPage, DevInspectArgs, DevInspectResults,
    DryRunTransactionBlockResponse, DynamicFieldPage, IotaData, IotaGetPastObjectRequest,
    IotaMoveNormalizedModule, IotaObjectDataOptions, IotaObjectResponse, IotaObjectResponseQuery,
    IotaPastObjectResponse, IotaTransactionBlockEffects, IotaTransactionBlockResponse,
    IotaTransactionBlockResponseOptions, IotaTransactionBlockResponseQuery, ObjectsPage,
    ProtocolConfigResponse, TransactionBlocksPage, TransactionFilter,
};
use iota_types::{
    base_types::{IotaAddress, ObjectID, SequenceNumber, TransactionDigest},
    dynamic_field::DynamicFieldName,
    iota_serde::BigInt,
    messages_checkpoint::CheckpointSequenceNumber,
    transaction::{TransactionData, TransactionKind},
};
use jsonrpsee::core::client::Subscription;

use crate::{
    RpcClient,
    error::{Error, IotaRpcResult},
};

/// Defines methods for retrieving data about objects and transactions.
#[derive(Debug)]
pub struct ReadApi {
    api: Arc<RpcClient>,
}

impl ReadApi {
    pub(crate) fn new(api: Arc<RpcClient>) -> Self {
        Self { api }
    }

    /// Get the objects owned by the given address.
    /// Results are paginated.
    ///
    /// Note that if the address owns more than
    /// [`QUERY_MAX_RESULT_LIMIT`](iota_json_rpc_api::QUERY_MAX_RESULT_LIMIT)
    /// objects (default is 50), the pagination may not be accurate as the
    /// previous page may have been updated before the next page is fetched.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use std::str::FromStr;
    ///
    /// use iota_sdk::IotaClientBuilder;
    /// use iota_types::base_types::IotaAddress;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), anyhow::Error> {
    ///     let iota = IotaClientBuilder::default().build_testnet().await?;
    ///     let address = IotaAddress::from_str("0x0000....0000")?;
    ///     let owned_objects = iota
    ///         .read_api()
    ///         .get_owned_objects(address, None, None, None)
    ///         .await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn get_owned_objects(
        &self,
        address: IotaAddress,
        query: impl Into<Option<IotaObjectResponseQuery>>,
        cursor: impl Into<Option<ObjectID>>,
        limit: impl Into<Option<usize>>,
    ) -> IotaRpcResult<ObjectsPage> {
        Ok(self
            .api
            .http
            .get_owned_objects(address, query.into(), cursor.into(), limit.into())
            .await?)
    }

    /// Get the dynamic fields owned by the given [ObjectID].
    /// Results are paginated.
    ///
    /// If the field is a dynamic field, this method returns the ID of the Field
    /// object, which contains both the name and the value.
    ///
    /// If the field is a dynamic object field, it returns the ID of the Object,
    /// which is the value of the field.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use std::str::FromStr;
    ///
    /// use iota_sdk::IotaClientBuilder;
    /// use iota_types::base_types::{IotaAddress, ObjectID};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), anyhow::Error> {
    ///     let iota = IotaClientBuilder::default().build_testnet().await?;
    ///     let address = IotaAddress::from_str("0x0000....0000")?;
    ///     let owned_objects = iota
    ///         .read_api()
    ///         .get_owned_objects(address, None, None, None)
    ///         .await?;
    ///     // this code example assumes that there are previous owned objects
    ///     let object = owned_objects
    ///         .data
    ///         .get(0)
    ///         .expect(&format!("No owned objects for this address {}", address));
    ///     let object_data = object.data.as_ref().expect(&format!(
    ///         "No object data for this IotaObjectResponse {:?}",
    ///         object
    ///     ));
    ///     let object_id = object_data.object_id;
    ///     let dynamic_fields = iota
    ///         .read_api()
    ///         .get_dynamic_fields(object_id, None, None)
    ///         .await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn get_dynamic_fields(
        &self,
        object_id: ObjectID,
        cursor: impl Into<Option<ObjectID>>,
        limit: impl Into<Option<usize>>,
    ) -> IotaRpcResult<DynamicFieldPage> {
        Ok(self
            .api
            .http
            .get_dynamic_fields(object_id, cursor.into(), limit.into())
            .await?)
    }

    /// Get information for a specified dynamic field object by its parent
    /// object ID and field name.
    pub async fn get_dynamic_field_object(
        &self,
        parent_object_id: ObjectID,
        name: DynamicFieldName,
    ) -> IotaRpcResult<IotaObjectResponse> {
        Ok(self
            .api
            .http
            .get_dynamic_field_object(parent_object_id, name)
            .await?)
    }

    /// Get information for a specified dynamic field object by its parent
    /// object ID and field name with options.
    pub async fn get_dynamic_field_object_v2(
        &self,
        parent_object_id: ObjectID,
        name: DynamicFieldName,
        options: impl Into<Option<IotaObjectDataOptions>>,
    ) -> IotaRpcResult<IotaObjectResponse> {
        Ok(self
            .api
            .http
            .get_dynamic_field_object_v2(parent_object_id, name, options.into())
            .await?)
    }

    /// Get a parsed past object and version for the provided object ID.
    ///
    /// An object's version increases when the object is mutated, though it is
    /// not guaranteed that it increases always by 1. A past object can be used
    /// to understand how the object changed over time, i.e. what was the total
    /// balance at a specific version.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use std::str::FromStr;
    ///
    /// use iota_json_rpc_types::IotaObjectDataOptions;
    /// use iota_sdk::IotaClientBuilder;
    /// use iota_types::base_types::{IotaAddress, ObjectID};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), anyhow::Error> {
    ///     let iota = IotaClientBuilder::default().build_testnet().await?;
    ///     let address = IotaAddress::from_str("0x0000....0000")?;
    ///     let owned_objects = iota
    ///         .read_api()
    ///         .get_owned_objects(address, None, None, None)
    ///         .await?;
    ///     // this code example assumes that there are previous owned objects
    ///     let object = owned_objects
    ///         .data
    ///         .get(0)
    ///         .expect(&format!("No owned objects for this address {}", address));
    ///     let object_data = object.data.as_ref().expect(&format!(
    ///         "No object data for this IotaObjectResponse {:?}",
    ///         object
    ///     ));
    ///     let object_id = object_data.object_id;
    ///     let version = object_data.version;
    ///     let past_object = iota
    ///         .read_api()
    ///         .try_get_parsed_past_object(
    ///             object_id,
    ///             version,
    ///             IotaObjectDataOptions {
    ///                 show_type: true,
    ///                 show_owner: true,
    ///                 show_previous_transaction: true,
    ///                 show_display: true,
    ///                 show_content: true,
    ///                 show_bcs: true,
    ///                 show_storage_rebate: true,
    ///             },
    ///         )
    ///         .await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn try_get_parsed_past_object(
        &self,
        object_id: ObjectID,
        version: SequenceNumber,
        options: IotaObjectDataOptions,
    ) -> IotaRpcResult<IotaPastObjectResponse> {
        Ok(self
            .api
            .http
            .try_get_past_object(object_id, version, Some(options))
            .await?)
    }

    /// Get a list of parsed past objects.
    ///
    /// See [Self::try_get_parsed_past_object] for more details about past
    /// objects.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use std::str::FromStr;
    ///
    /// use iota_json_rpc_types::{IotaGetPastObjectRequest, IotaObjectDataOptions};
    /// use iota_sdk::IotaClientBuilder;
    /// use iota_types::base_types::{IotaAddress, ObjectID};
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), anyhow::Error> {
    ///     let iota = IotaClientBuilder::default().build_testnet().await?;
    ///     let address = IotaAddress::from_str("0x0000....0000")?;
    ///     let owned_objects = iota
    ///         .read_api()
    ///         .get_owned_objects(address, None, None, None)
    ///         .await?;
    ///     // this code example assumes that there are previous owned objects
    ///     let object = owned_objects
    ///         .data
    ///         .get(0)
    ///         .expect(&format!("No owned objects for this address {}", address));
    ///     let object_data = object.data.as_ref().expect(&format!(
    ///         "No object data for this IotaObjectResponse {:?}",
    ///         object
    ///     ));
    ///     let object_id = object_data.object_id;
    ///     let version = object_data.version;
    ///     let past_object = iota
    ///         .read_api()
    ///         .try_get_parsed_past_object(
    ///             object_id,
    ///             version,
    ///             IotaObjectDataOptions {
    ///                 show_type: true,
    ///                 show_owner: true,
    ///                 show_previous_transaction: true,
    ///                 show_display: true,
    ///                 show_content: true,
    ///                 show_bcs: true,
    ///                 show_storage_rebate: true,
    ///             },
    ///         )
    ///         .await?;
    ///     let past_object = past_object.into_object()?;
    ///     let multi_past_object = iota
    ///         .read_api()
    ///         .try_multi_get_parsed_past_object(
    ///             vec![IotaGetPastObjectRequest {
    ///                 object_id: past_object.object_id,
    ///                 version: past_object.version,
    ///             }],
    ///             IotaObjectDataOptions {
    ///                 show_type: true,
    ///                 show_owner: true,
    ///                 show_previous_transaction: true,
    ///                 show_display: true,
    ///                 show_content: true,
    ///                 show_bcs: true,
    ///                 show_storage_rebate: true,
    ///             },
    ///         )
    ///         .await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn try_multi_get_parsed_past_object(
        &self,
        past_objects: Vec<IotaGetPastObjectRequest>,
        options: IotaObjectDataOptions,
    ) -> IotaRpcResult<Vec<IotaPastObjectResponse>> {
        Ok(self
            .api
            .http
            .try_multi_get_past_objects(past_objects, Some(options))
            .await?)
    }

    /// Get an object by object ID with optional fields enabled by
    /// [IotaObjectDataOptions].
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use std::str::FromStr;
    ///
    /// use iota_json_rpc_types::IotaObjectDataOptions;
    /// use iota_sdk::IotaClientBuilder;
    /// use iota_types::base_types::IotaAddress;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), anyhow::Error> {
    ///     let iota = IotaClientBuilder::default().build_testnet().await?;
    ///     let address = IotaAddress::from_str("0x0000....0000")?;
    ///     let owned_objects = iota
    ///         .read_api()
    ///         .get_owned_objects(address, None, None, None)
    ///         .await?;
    ///     // this code example assumes that there are previous owned objects
    ///     let object = owned_objects
    ///         .data
    ///         .get(0)
    ///         .expect(&format!("No owned objects for this address {}", address));
    ///     let object_data = object.data.as_ref().expect(&format!(
    ///         "No object data for this IotaObjectResponse {:?}",
    ///         object
    ///     ));
    ///     let object_id = object_data.object_id;
    ///     let object = iota
    ///         .read_api()
    ///         .get_object_with_options(
    ///             object_id,
    ///             IotaObjectDataOptions {
    ///                 show_type: true,
    ///                 show_owner: true,
    ///                 show_previous_transaction: true,
    ///                 show_display: true,
    ///                 show_content: true,
    ///                 show_bcs: true,
    ///                 show_storage_rebate: true,
    ///             },
    ///         )
    ///         .await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn get_object_with_options(
        &self,
        object_id: ObjectID,
        options: IotaObjectDataOptions,
    ) -> IotaRpcResult<IotaObjectResponse> {
        Ok(self.api.http.get_object(object_id, Some(options)).await?)
    }

    /// Get a list of objects by their object IDs with optional fields enabled
    /// by [IotaObjectDataOptions].
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use std::str::FromStr;
    ///
    /// use iota_json_rpc_types::IotaObjectDataOptions;
    /// use iota_sdk::IotaClientBuilder;
    /// use iota_types::base_types::IotaAddress;
    /// #[tokio::main]
    /// async fn main() -> Result<(), anyhow::Error> {
    ///     let iota = IotaClientBuilder::default().build_testnet().await?;
    ///     let address = IotaAddress::from_str("0x0000....0000")?;
    ///     let owned_objects = iota
    ///         .read_api()
    ///         .get_owned_objects(address, None, None, None)
    ///         .await?;
    ///     // this code example assumes that there are previous owned objects
    ///     let object = owned_objects
    ///         .data
    ///         .get(0)
    ///         .expect(&format!("No owned objects for this address {}", address));
    ///     let object_data = object.data.as_ref().expect(&format!(
    ///         "No object data for this IotaObjectResponse {:?}",
    ///         object
    ///     ));
    ///     let object_id = object_data.object_id;
    ///     let object_ids = vec![object_id]; // and other object ids
    ///     let object = iota
    ///         .read_api()
    ///         .multi_get_object_with_options(
    ///             object_ids,
    ///             IotaObjectDataOptions {
    ///                 show_type: true,
    ///                 show_owner: true,
    ///                 show_previous_transaction: true,
    ///                 show_display: true,
    ///                 show_content: true,
    ///                 show_bcs: true,
    ///                 show_storage_rebate: true,
    ///             },
    ///         )
    ///         .await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn multi_get_object_with_options(
        &self,
        object_ids: Vec<ObjectID>,
        options: IotaObjectDataOptions,
    ) -> IotaRpcResult<Vec<IotaObjectResponse>> {
        Ok(self
            .api
            .http
            .multi_get_objects(object_ids, Some(options))
            .await?)
    }

    /// Get a [bcs] serialized object's bytes by object ID.
    pub async fn get_move_object_bcs(&self, object_id: ObjectID) -> IotaRpcResult<Vec<u8>> {
        let resp = self
            .get_object_with_options(object_id, IotaObjectDataOptions::default().with_bcs())
            .await?
            .into_object()
            .map_err(|e| {
                Error::Data(format!("Can't get bcs of object {:?}: {:?}", object_id, e))
            })?;
        // unwrap: requested bcs data
        let move_object = resp.bcs.unwrap();
        let raw_move_obj = move_object.try_into_move().ok_or(Error::Data(format!(
            "Object {:?} is not a MoveObject",
            object_id
        )))?;
        Ok(raw_move_obj.bcs_bytes)
    }

    /// Get the total number of transaction blocks known to server.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use iota_sdk::IotaClientBuilder;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), anyhow::Error> {
    ///     let iota = IotaClientBuilder::default().build_testnet().await?;
    ///     let total_transaction_blocks = iota.read_api().get_total_transaction_blocks().await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn get_total_transaction_blocks(&self) -> IotaRpcResult<u64> {
        Ok(*self.api.http.get_total_transaction_blocks().await?)
    }

    /// Get a transaction and its effects by its digest with optional fields
    /// enabled by [IotaTransactionBlockResponseOptions].
    pub async fn get_transaction_with_options(
        &self,
        digest: TransactionDigest,
        options: IotaTransactionBlockResponseOptions,
    ) -> IotaRpcResult<IotaTransactionBlockResponse> {
        Ok(self
            .api
            .http
            .get_transaction_block(digest, Some(options))
            .await?)
    }

    /// Get a list of transactions and their effects by their digests with
    /// optional fields enabled by [IotaTransactionBlockResponseOptions].
    pub async fn multi_get_transactions_with_options(
        &self,
        digests: Vec<TransactionDigest>,
        options: IotaTransactionBlockResponseOptions,
    ) -> IotaRpcResult<Vec<IotaTransactionBlockResponse>> {
        Ok(self
            .api
            .http
            .multi_get_transaction_blocks(digests, Some(options))
            .await?)
    }

    /// Get filtered transaction blocks information.
    /// Results are paginated.
    pub async fn query_transaction_blocks(
        &self,
        query: IotaTransactionBlockResponseQuery,
        cursor: impl Into<Option<TransactionDigest>>,
        limit: impl Into<Option<usize>>,
        descending_order: bool,
    ) -> IotaRpcResult<TransactionBlocksPage> {
        Ok(self
            .api
            .http
            .query_transaction_blocks(query, cursor.into(), limit.into(), Some(descending_order))
            .await?)
    }

    /// Get the first four bytes of the chain's genesis checkpoint digest in hex
    /// format.
    pub async fn get_chain_identifier(&self) -> IotaRpcResult<String> {
        Ok(self.api.http.get_chain_identifier().await?)
    }

    /// Get a checkpoint by its ID.
    pub async fn get_checkpoint(&self, id: CheckpointId) -> IotaRpcResult<Checkpoint> {
        Ok(self.api.http.get_checkpoint(id).await?)
    }

    /// Return a list of checkpoints.
    /// Results are paginated.
    pub async fn get_checkpoints(
        &self,
        cursor: impl Into<Option<BigInt<u64>>>,
        limit: impl Into<Option<usize>>,
        descending_order: bool,
    ) -> IotaRpcResult<CheckpointPage> {
        Ok(self
            .api
            .http
            .get_checkpoints(cursor.into(), limit.into(), descending_order)
            .await?)
    }

    /// Get the sequence number of the latest checkpoint that has been executed.
    pub async fn get_latest_checkpoint_sequence_number(
        &self,
    ) -> IotaRpcResult<CheckpointSequenceNumber> {
        Ok(*self
            .api
            .http
            .get_latest_checkpoint_sequence_number()
            .await?)
    }

    /// Get a stream of transactions.
    pub fn get_transactions_stream(
        &self,
        query: IotaTransactionBlockResponseQuery,
        cursor: impl Into<Option<TransactionDigest>>,
        descending_order: bool,
    ) -> impl Stream<Item = IotaTransactionBlockResponse> + '_ {
        let cursor = cursor.into();

        stream::unfold(
            (vec![], cursor, true, query),
            move |(mut data, cursor, first, query)| async move {
                if let Some(item) = data.pop() {
                    Some((item, (data, cursor, false, query)))
                } else if (cursor.is_none() && first) || cursor.is_some() {
                    let page = self
                        .query_transaction_blocks(
                            query.clone(),
                            cursor,
                            Some(100),
                            descending_order,
                        )
                        .await
                        .ok()?;
                    let mut data = page.data;
                    data.reverse();
                    data.pop()
                        .map(|item| (item, (data, page.next_cursor, false, query)))
                } else {
                    None
                }
            },
        )
    }

    /// Subscribe to a stream of transactions.
    ///
    /// This is only available through WebSockets.
    pub async fn subscribe_transaction(
        &self,
        filter: TransactionFilter,
    ) -> IotaRpcResult<impl Stream<Item = IotaRpcResult<IotaTransactionBlockEffects>>> {
        let Some(c) = &self.api.ws else {
            return Err(Error::Subscription(
                "Subscription only supported by WebSocket client.".to_string(),
            ));
        };
        let subscription: Subscription<IotaTransactionBlockEffects> =
            c.subscribe_transaction(filter).await?;
        Ok(subscription.map(|item| Ok(item?)))
    }

    /// Get move modules by package ID, keyed by name.
    pub async fn get_normalized_move_modules_by_package(
        &self,
        package: ObjectID,
    ) -> IotaRpcResult<BTreeMap<String, IotaMoveNormalizedModule>> {
        Ok(self
            .api
            .http
            .get_normalized_move_modules_by_package(package)
            .await?)
    }

    // TODO(devx): we can probably cache this given an epoch
    /// Get the reference gas price.
    pub async fn get_reference_gas_price(&self) -> IotaRpcResult<u64> {
        Ok(*self.api.http.get_reference_gas_price().await?)
    }

    /// Dry run a transaction block given the provided transaction data.
    ///
    /// This simulates running the transaction, including all standard checks,
    /// without actually running it. This is useful for estimating the gas fees
    /// of a transaction before executing it. You can also use it to identify
    /// any side-effects of a transaction before you execute it on the network.
    pub async fn dry_run_transaction_block(
        &self,
        tx: TransactionData,
    ) -> IotaRpcResult<DryRunTransactionBlockResponse> {
        Ok(self
            .api
            .http
            .dry_run_transaction_block(Base64::from_bytes(&bcs::to_bytes(&tx)?))
            .await?)
    }

    /// Use this function to inspect the current state of the network by running
    /// a programmable transaction block without committing its effects on
    /// chain.
    ///
    /// Unlike a dry run, this method will not validate whether the transaction
    /// block would succeed or fail under normal circumstances, e.g.:
    ///
    /// - Transaction inputs are not checked for ownership (i.e. you can
    ///   construct calls involving objects you do not own)
    /// - Calls are not checked for visibility (you can call private functions
    ///   on modules)
    /// - Inputs of any type can be constructed and passed in, including coins
    ///   and other objects that would usually need to be constructed with a
    ///   move call
    /// - Function returns do not need to be used, even if they do not have
    ///   `drop`
    ///
    /// This method's output includes a breakdown of results returned by every
    /// transaction in the block, as well as the transaction's effects.
    ///
    /// To run an accurate simulation of a transaction and understand whether
    /// it will successfully validate and run, use
    /// [Self::dry_run_transaction_block] instead.
    pub async fn dev_inspect_transaction_block(
        &self,
        sender_address: IotaAddress,
        tx: TransactionKind,
        gas_price: impl Into<Option<BigInt<u64>>>,
        epoch: impl Into<Option<BigInt<u64>>>,
        additional_args: impl Into<Option<DevInspectArgs>>,
    ) -> IotaRpcResult<DevInspectResults> {
        Ok(self
            .api
            .http
            .dev_inspect_transaction_block(
                sender_address,
                Base64::from_bytes(&bcs::to_bytes(&tx)?),
                gas_price.into(),
                epoch.into(),
                additional_args.into(),
            )
            .await?)
    }

    /// Get the protocol config by version.
    ///
    /// The version defaults to the current version.
    pub async fn get_protocol_config(
        &self,
        version: impl Into<Option<BigInt<u64>>>,
    ) -> IotaRpcResult<ProtocolConfigResponse> {
        Ok(self.api.http.get_protocol_config(version.into()).await?)
    }

    /// Get an object by ID before the given version.
    pub async fn try_get_object_before_version(
        &self,
        object_id: ObjectID,
        version: SequenceNumber,
    ) -> IotaRpcResult<IotaPastObjectResponse> {
        Ok(self
            .api
            .http
            .try_get_object_before_version(object_id, version)
            .await?)
    }
}
