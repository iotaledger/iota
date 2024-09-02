// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeMap, sync::Arc};

use fastcrypto::encoding::Base64;
use futures::{stream, StreamExt};
use futures_core::Stream;
use iota_json_rpc_api::{
    GovernanceReadApiClient, IndexerApiClient, MoveUtilsClient, ReadApiClient, WriteApiClient,
};
use iota_json_rpc_types::{
    Checkpoint, CheckpointId, CheckpointPage, DevInspectArgs, DevInspectResults,
    DryRunTransactionBlockResponse, DynamicFieldPage, IotaCommittee, IotaData,
    IotaGetPastObjectRequest, IotaLoadedChildObjectsResponse, IotaMoveNormalizedModule,
    IotaObjectDataOptions, IotaObjectResponse, IotaObjectResponseQuery, IotaPastObjectResponse,
    IotaTransactionBlockEffects, IotaTransactionBlockResponse, IotaTransactionBlockResponseOptions,
    IotaTransactionBlockResponseQuery, ObjectsPage, ProtocolConfigResponse, TransactionBlocksPage,
    TransactionFilter,
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
    error::{Error, IotaRpcResult},
    RpcClient,
};

/// The main read API structure with functions for retrieving data about
/// different objects and transactions
#[derive(Debug)]
pub struct ReadApi {
    api: Arc<RpcClient>,
}

impl ReadApi {
    pub(crate) fn new(api: Arc<RpcClient>) -> Self {
        Self { api }
    }
    /// Return a paginated response with the objects owned by the given address,
    /// or an error upon failure.
    ///
    /// Note that if the address owns more than `QUERY_MAX_RESULT_LIMIT` objects
    /// (default is 50), the pagination is not accurate, because previous
    /// page may have been updated when the next page is fetched.
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
    ///     let iota = IotaClientBuilder::default().build_localnet().await?;
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
        query: Option<IotaObjectResponseQuery>,
        cursor: Option<ObjectID>,
        limit: Option<usize>,
    ) -> IotaRpcResult<ObjectsPage> {
        Ok(self
            .api
            .http
            .get_owned_objects(address, query, cursor, limit)
            .await?)
    }

    /// Return a paginated response with the dynamic fields owned by the given
    /// [ObjectID], or an error upon failure.
    ///
    /// The return type is a list of `DynamicFieldInfo` objects, where the field
    /// name is always present, represented as a Move `Value`.
    ///
    /// If the field is a dynamic field, returns the ID of the Field object
    /// (which contains both the name and the value). If the field is a
    /// dynamic object field, it returns the ID of the Object (the value of the
    /// field).
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
    ///     let iota = IotaClientBuilder::default().build_localnet().await?;
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
        cursor: Option<ObjectID>,
        limit: Option<usize>,
    ) -> IotaRpcResult<DynamicFieldPage> {
        Ok(self
            .api
            .http
            .get_dynamic_fields(object_id, cursor, limit)
            .await?)
    }

    /// Return the dynamic field object information for a specified object.
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

    /// Return a parsed past object for the provided [ObjectID] and version, or
    /// an error upon failure.
    ///
    /// An object's version increases (though it is not guaranteed that it
    /// increases always by 1) when the object is mutated. A past object can
    /// be used to understand how the object changed over time,
    /// i.e. what was the total balance at a specific version.
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
    ///     let iota = IotaClientBuilder::default().build_localnet().await?;
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

    /// Return a list of [IotaPastObjectResponse] objects, or an error upon
    /// failure.
    ///
    /// See [this function](ReadApi::try_get_parsed_past_object) for more
    /// details about past objects.
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
    ///     let iota = IotaClientBuilder::default().build_localnet().await?;
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

    /// Return a [IotaObjectResponse] based on the provided [ObjectID] and
    /// [IotaObjectDataOptions], or an error upon failure.
    ///
    /// The [IotaObjectResponse] contains two fields:
    /// 1) `data` for the object's data (see
    ///    [IotaObjectData](iota_json_rpc_types::IotaObjectData)),
    /// 2) `error` for the error (if any) (see
    ///    [IotaObjectResponseError](iota_types::error::IotaObjectResponseError)).
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
    ///     let iota = IotaClientBuilder::default().build_localnet().await?;
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

    /// Return a list of [IotaObjectResponse] from the given vector of
    /// [ObjectID]s and [IotaObjectDataOptions], or an error upon failure.
    ///
    /// If only one object is needed, use the
    /// [get_object_with_options](ReadApi::get_object_with_options) function
    /// instead.
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
    ///     let iota = IotaClientBuilder::default().build_localnet().await?;
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

    /// Return An object's bcs content [`Vec<u8>`] based on the provided
    /// [ObjectID], or an error upon failure.
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

    /// Return the total number of transaction blocks known to server, or an
    /// error upon failure.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use iota_sdk::IotaClientBuilder;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), anyhow::Error> {
    ///     let iota = IotaClientBuilder::default().build_localnet().await?;
    ///     let total_transaction_blocks = iota.read_api().get_total_transaction_blocks().await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn get_total_transaction_blocks(&self) -> IotaRpcResult<u64> {
        Ok(*self.api.http.get_total_transaction_blocks().await?)
    }

    /// Return a transaction and its effects in a [IotaTransactionBlockResponse]
    /// based on its [TransactionDigest], or an error upon failure.
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
    /// Return a list of [IotaTransactionBlockResponse] based on the given
    /// vector of [TransactionDigest], or an error upon failure.
    ///
    /// If only one transaction data is needed, use the
    /// [get_transaction_with_options](ReadApi::get_transaction_with_options)
    /// function instead.
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

    /// Return the [IotaCommittee] information for the provided `epoch`, or an
    /// error upon failure.
    ///
    /// The [IotaCommittee] contains the validators list and their information
    /// (name and stakes).
    ///
    /// The argument `epoch` is either a known epoch id or `None` for the
    /// current epoch.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use iota_sdk::IotaClientBuilder;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), anyhow::Error> {
    ///     let iota = IotaClientBuilder::default().build_localnet().await?;
    ///     let committee_info = iota.read_api().get_committee_info(None).await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn get_committee_info(
        &self,
        epoch: Option<BigInt<u64>>,
    ) -> IotaRpcResult<IotaCommittee> {
        Ok(self.api.http.get_committee_info(epoch).await?)
    }

    /// Return a paginated response with all transaction blocks information, or
    /// an error upon failure.
    pub async fn query_transaction_blocks(
        &self,
        query: IotaTransactionBlockResponseQuery,
        cursor: Option<TransactionDigest>,
        limit: Option<usize>,
        descending_order: bool,
    ) -> IotaRpcResult<TransactionBlocksPage> {
        Ok(self
            .api
            .http
            .query_transaction_blocks(query, cursor, limit, Some(descending_order))
            .await?)
    }

    /// Return the first four bytes of the chain's genesis checkpoint digest, or
    /// an error upon failure.
    pub async fn get_chain_identifier(&self) -> IotaRpcResult<String> {
        Ok(self.api.http.get_chain_identifier().await?)
    }

    /// Return a checkpoint, or an error upon failure.
    ///
    /// A Iota checkpoint is a sequence of transaction sets that a quorum of
    /// validators agree upon as having been executed within the Iota system.
    pub async fn get_checkpoint(&self, id: CheckpointId) -> IotaRpcResult<Checkpoint> {
        Ok(self.api.http.get_checkpoint(id).await?)
    }

    /// Return a paginated list of checkpoints, or an error upon failure.
    pub async fn get_checkpoints(
        &self,
        cursor: Option<BigInt<u64>>,
        limit: Option<usize>,
        descending_order: bool,
    ) -> IotaRpcResult<CheckpointPage> {
        Ok(self
            .api
            .http
            .get_checkpoints(cursor, limit, descending_order)
            .await?)
    }

    /// Return the sequence number of the latest checkpoint that has been
    /// executed, or an error upon failure.
    pub async fn get_latest_checkpoint_sequence_number(
        &self,
    ) -> IotaRpcResult<CheckpointSequenceNumber> {
        Ok(*self
            .api
            .http
            .get_latest_checkpoint_sequence_number()
            .await?)
    }

    /// Return a stream of [IotaTransactionBlockResponse], or an error upon
    /// failure.
    pub fn get_transactions_stream(
        &self,
        query: IotaTransactionBlockResponseQuery,
        cursor: Option<TransactionDigest>,
        descending_order: bool,
    ) -> impl Stream<Item = IotaTransactionBlockResponse> + '_ {
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

    /// Return a map consisting of the move package name and the normalized
    /// module, or an error upon failure.
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
    /// Return the reference gas price, or an error upon failure.
    pub async fn get_reference_gas_price(&self) -> IotaRpcResult<u64> {
        Ok(*self.api.http.get_reference_gas_price().await?)
    }

    /// Dry run a transaction block given the provided transaction data. Returns
    /// an error upon failure.
    ///
    /// Simulate running the transaction, including all standard checks, without
    /// actually running it. This is useful for estimating the gas fees of a
    /// transaction before executing it. You can also use it to identify any
    /// side-effects of a transaction before you execute it on the network.
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

    /// Return the inspection of the transaction block, or an error upon
    /// failure.
    ///
    /// Use this function to inspect the current state of the network by running
    /// a programmable transaction block without committing its effects on
    /// chain.  Unlike
    /// [dry_run_transaction_block](ReadApi::dry_run_transaction_block),
    /// dev inspect will not validate whether the transaction block
    /// would succeed or fail under normal circumstances, e.g.:
    ///
    /// - Transaction inputs are not checked for ownership (i.e. you can
    ///   construct calls involving objects you do not own).
    /// - Calls are not checked for visibility (you can call private functions
    ///   on modules)
    /// - Inputs of any type can be constructed and passed in, (including Coins
    ///   and other objects that would usually need to be constructed with a
    ///   move call).
    /// - Function returns do not need to be used, even if they do not have
    ///   `drop`.
    ///
    /// Dev inspect's output includes a breakdown of results returned by every
    /// transaction in the block, as well as the transaction's effects.
    ///
    /// To run an accurate simulation of a transaction and understand whether
    /// it will successfully validate and run,
    /// use the [dry_run_transaction_block](ReadApi::dry_run_transaction_block)
    /// function instead.
    pub async fn dev_inspect_transaction_block(
        &self,
        sender_address: IotaAddress,
        tx: TransactionKind,
        gas_price: Option<BigInt<u64>>,
        epoch: Option<BigInt<u64>>,
        additional_args: Option<DevInspectArgs>,
    ) -> IotaRpcResult<DevInspectResults> {
        Ok(self
            .api
            .http
            .dev_inspect_transaction_block(
                sender_address,
                Base64::from_bytes(&bcs::to_bytes(&tx)?),
                gas_price,
                epoch,
                additional_args,
            )
            .await?)
    }

    /// Return the loaded child objects response for the provided digest, or an
    /// error upon failure.
    ///
    /// Loaded child objects
    /// ([IotaLoadedChildObject](iota_json_rpc_types::IotaLoadedChildObject))
    /// are the non-input objects that the transaction at the digest loaded
    /// in response to dynamic field accesses.
    pub async fn get_loaded_child_objects(
        &self,
        digest: TransactionDigest,
    ) -> IotaRpcResult<IotaLoadedChildObjectsResponse> {
        Ok(self.api.http.get_loaded_child_objects(digest).await?)
    }

    /// Return the protocol config, or an error upon failure.
    pub async fn get_protocol_config(
        &self,
        version: Option<BigInt<u64>>,
    ) -> IotaRpcResult<ProtocolConfigResponse> {
        Ok(self.api.http.get_protocol_config(version).await?)
    }
}
