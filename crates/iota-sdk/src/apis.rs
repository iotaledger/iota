// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::BTreeMap,
    future,
    sync::Arc,
    time::{Duration, Instant},
};

use fastcrypto::encoding::Base64;
use futures::{stream, StreamExt};
use futures_core::Stream;
use iota_json_rpc_api::{
    CoinReadApiClient, GovernanceReadApiClient, IndexerApiClient, MoveUtilsClient, ReadApiClient,
    WriteApiClient,
};
use iota_json_rpc_types::{
    Balance, Checkpoint, CheckpointId, CheckpointPage, Coin, CoinPage, DelegatedStake,
    DevInspectArgs, DevInspectResults, DryRunTransactionBlockResponse, DynamicFieldPage,
    EventFilter, EventPage, IotaCoinMetadata, IotaCommittee, IotaData, IotaEvent,
    IotaGetPastObjectRequest, IotaMoveNormalizedModule, IotaObjectDataOptions, IotaObjectResponse,
    IotaObjectResponseQuery, IotaPastObjectResponse, IotaTransactionBlockEffects,
    IotaTransactionBlockResponse, IotaTransactionBlockResponseOptions,
    IotaTransactionBlockResponseQuery, ObjectsPage, ProtocolConfigResponse, TransactionBlocksPage,
    TransactionFilter,
};
use iota_types::{
    balance::Supply,
    base_types::{IotaAddress, ObjectID, SequenceNumber, TransactionDigest},
    dynamic_field::DynamicFieldName,
    event::EventID,
    iota_serde::BigInt,
    iota_system_state::iota_system_state_summary::IotaSystemStateSummary,
    messages_checkpoint::CheckpointSequenceNumber,
    quorum_driver_types::ExecuteTransactionRequestType,
    transaction::{Transaction, TransactionData, TransactionKind},
};
use jsonrpsee::core::client::Subscription;

use crate::{
    error::{Error, IotaRpcResult},
    RpcClient,
};

const WAIT_FOR_LOCAL_EXECUTION_TIMEOUT: Duration = Duration::from_secs(60);
const WAIT_FOR_LOCAL_EXECUTION_DELAY: Duration = Duration::from_millis(200);
const WAIT_FOR_LOCAL_EXECUTION_INTERVAL: Duration = Duration::from_secs(2);

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
                Error::DataError(format!("Can't get bcs of object {:?}: {:?}", object_id, e))
            })?;
        // unwrap: requested bcs data
        let move_object = resp.bcs.unwrap();
        let raw_move_obj = move_object.try_into_move().ok_or(Error::DataError(format!(
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
    /// validators agree upon as having been executed within the Iota
    /// system.
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

    /// Return the protocol config, or an error upon failure.
    pub async fn get_protocol_config(
        &self,
        version: Option<BigInt<u64>>,
    ) -> IotaRpcResult<ProtocolConfigResponse> {
        Ok(self.api.http.get_protocol_config(version).await?)
    }

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

/// Coin Read API provides the functionality needed to get information from the
/// Iota network regarding the coins owned by an address.
#[derive(Debug, Clone)]
pub struct CoinReadApi {
    api: Arc<RpcClient>,
}

impl CoinReadApi {
    pub(crate) fn new(api: Arc<RpcClient>) -> Self {
        Self { api }
    }

    /// Return a paginated response with the coins for the given address, or an
    /// error upon failure.
    ///
    /// The coins can be filtered by `coin_type` (e.g.,
    /// 0x168da5bf1f48dafc111b0a488fa454aca95e0b5e::usdc::USDC)
    /// or use `None` for the default `Coin<IOTA>`.
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
    ///     let coins = iota
    ///         .coin_read_api()
    ///         .get_coins(address, None, None, None)
    ///         .await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn get_coins(
        &self,
        owner: IotaAddress,
        coin_type: Option<String>,
        cursor: Option<ObjectID>,
        limit: Option<usize>,
    ) -> IotaRpcResult<CoinPage> {
        Ok(self
            .api
            .http
            .get_coins(owner, coin_type, cursor, limit)
            .await?)
    }
    /// Return a paginated response with all the coins for the given address, or
    /// an error upon failure.
    ///
    /// This function includes all coins. If needed to filter by coin type, use
    /// the `get_coins` method instead.
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
    ///     let coins = iota
    ///         .coin_read_api()
    ///         .get_all_coins(address, None, None)
    ///         .await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn get_all_coins(
        &self,
        owner: IotaAddress,
        cursor: Option<ObjectID>,
        limit: Option<usize>,
    ) -> IotaRpcResult<CoinPage> {
        Ok(self.api.http.get_all_coins(owner, cursor, limit).await?)
    }

    /// Return the coins for the given address as a stream.
    ///
    /// The coins can be filtered by `coin_type` (e.g.,
    /// 0x168da5bf1f48dafc111b0a488fa454aca95e0b5e::usdc::USDC)
    /// or use `None` for the default `Coin<IOTA>`.
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
    ///     let coins = iota.coin_read_api().get_coins_stream(address, None);
    ///     Ok(())
    /// }
    /// ```
    pub fn get_coins_stream(
        &self,
        owner: IotaAddress,
        coin_type: Option<String>,
    ) -> impl Stream<Item = Coin> + '_ {
        stream::unfold(
            (
                vec![],
                // cursor
                None,
                // has_next_page
                true,
                coin_type,
            ),
            move |(mut data, cursor, has_next_page, coin_type)| async move {
                if let Some(item) = data.pop() {
                    Some((item, (data, cursor, /* has_next_page */ true, coin_type)))
                } else if has_next_page {
                    let page = self
                        .get_coins(owner, coin_type.clone(), cursor, Some(100))
                        .await
                        .ok()?;
                    let mut data = page.data;
                    data.reverse();
                    data.pop().map(|item| {
                        (
                            item,
                            (data, page.next_cursor, page.has_next_page, coin_type),
                        )
                    })
                } else {
                    None
                }
            },
        )
    }

    /// Return a list of coins for the given address, or an error upon failure.
    ///
    /// Note that the function selects coins to meet or exceed the requested
    /// `amount`. If that it is not possible, it will fail with an
    /// insufficient fund error.
    ///
    /// The coins can be filtered by `coin_type` (e.g.,
    /// 0x168da5bf1f48dafc111b0a488fa454aca95e0b5e::usdc::USDC)
    /// or use `None` to use the default `Coin<IOTA>`.
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
    ///     let coins = iota
    ///         .coin_read_api()
    ///         .select_coins(address, None, 5, vec![])
    ///         .await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn select_coins(
        &self,
        address: IotaAddress,
        coin_type: Option<String>,
        amount: u128,
        exclude: Vec<ObjectID>,
    ) -> IotaRpcResult<Vec<Coin>> {
        let mut total = 0u128;
        let coins = self
            .get_coins_stream(address, coin_type)
            .filter(|coin: &Coin| future::ready(!exclude.contains(&coin.coin_object_id)))
            .take_while(|coin: &Coin| {
                let ready = future::ready(total < amount);
                total += coin.balance as u128;
                ready
            })
            .collect::<Vec<_>>()
            .await;

        if total < amount {
            return Err(Error::InsufficientFund { address, amount });
        }
        Ok(coins)
    }

    /// Return the balance for the given coin type owned by address, or an error
    /// upon failure.
    ///
    /// Note that this function sums up all the balances of all the coins
    /// matching the given coin type. By default, if `coin_type` is set to
    /// `None`, it will use the default `Coin<IOTA>`.
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
    ///     let balance = iota.coin_read_api().get_balance(address, None).await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn get_balance(
        &self,
        owner: IotaAddress,
        coin_type: Option<String>,
    ) -> IotaRpcResult<Balance> {
        Ok(self.api.http.get_balance(owner, coin_type).await?)
    }

    /// Return a list of balances for each coin type owned by the given address,
    /// or an error upon failure.
    ///
    /// Note that this function groups the coins by coin type, and sums up all
    /// their balances.
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
    ///     let all_balances = iota.coin_read_api().get_all_balances(address).await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn get_all_balances(&self, owner: IotaAddress) -> IotaRpcResult<Vec<Balance>> {
        Ok(self.api.http.get_all_balances(owner).await?)
    }

    /// Return the coin metadata (name, symbol, description, decimals, etc.) for
    /// a given coin type, or an error upon failure.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use iota_sdk::IotaClientBuilder;
    /// #[tokio::main]
    /// async fn main() -> Result<(), anyhow::Error> {
    ///     let iota = IotaClientBuilder::default().build_localnet().await?;
    ///     let coin_metadata = iota
    ///         .coin_read_api()
    ///         .get_coin_metadata("0x2::iota::IOTA".to_string())
    ///         .await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn get_coin_metadata(
        &self,
        coin_type: String,
    ) -> IotaRpcResult<Option<IotaCoinMetadata>> {
        Ok(self.api.http.get_coin_metadata(coin_type).await?)
    }

    /// Return the total supply for a given coin type, or an error upon failure.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use iota_sdk::IotaClientBuilder;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), anyhow::Error> {
    ///     let iota = IotaClientBuilder::default().build_localnet().await?;
    ///     let total_supply = iota
    ///         .coin_read_api()
    ///         .get_total_supply("0x2::iota::IOTA".to_string())
    ///         .await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn get_total_supply(&self, coin_type: String) -> IotaRpcResult<Supply> {
        Ok(self.api.http.get_total_supply(coin_type).await?)
    }
}

/// Event API provides the functionality to fetch, query, or subscribe to events
/// on the Iota network.
#[derive(Clone)]
pub struct EventApi {
    api: Arc<RpcClient>,
}

impl EventApi {
    pub(crate) fn new(api: Arc<RpcClient>) -> Self {
        Self { api }
    }

    /// Return a stream of events, or an error upon failure.
    ///
    /// Subscription is only possible via WebSockets.
    /// For a list of possible event filters, see [EventFilter].
    ///
    /// # Examples
    ///
    /// ```rust, no_run
    /// use std::str::FromStr;
    ///
    /// use futures::StreamExt;
    /// use iota_json_rpc_types::EventFilter;
    /// use iota_sdk::IotaClientBuilder;
    /// use iota_types::base_types::IotaAddress;
    /// #[tokio::main]
    /// async fn main() -> Result<(), anyhow::Error> {
    ///     let iota = IotaClientBuilder::default()
    ///         .ws_url("wss://rpc.mainnet.iota.io:443")
    ///         .build("https://fullnode.mainnet.iota.io:443")
    ///         .await?;
    ///     let mut subscribe_all = iota
    ///         .event_api()
    ///         .subscribe_event(EventFilter::All(vec![]))
    ///         .await?;
    ///     loop {
    ///         println!("{:?}", subscribe_all.next().await);
    ///     }
    ///     Ok(())
    /// }
    /// ```
    pub async fn subscribe_event(
        &self,
        filter: EventFilter,
    ) -> IotaRpcResult<impl Stream<Item = IotaRpcResult<IotaEvent>>> {
        match &self.api.ws {
            Some(c) => {
                let subscription: Subscription<IotaEvent> = c.subscribe_event(filter).await?;
                Ok(subscription.map(|item| Ok(item?)))
            }
            _ => Err(Error::Subscription(
                "Subscription only supported by WebSocket client.".to_string(),
            )),
        }
    }

    /// Return a list of events for the given transaction digest, or an error
    /// upon failure.
    pub async fn get_events(&self, digest: TransactionDigest) -> IotaRpcResult<Vec<IotaEvent>> {
        Ok(self.api.http.get_events(digest).await?)
    }

    /// Return a paginated response with events for the given event filter, or
    /// an error upon failure.
    ///
    /// The ordering of the events can be set with the `descending_order`
    /// argument. For a list of possible event filters, see [EventFilter].
    pub async fn query_events(
        &self,
        query: EventFilter,
        cursor: Option<EventID>,
        limit: Option<usize>,
        descending_order: bool,
    ) -> IotaRpcResult<EventPage> {
        Ok(self
            .api
            .http
            .query_events(query, cursor, limit, Some(descending_order))
            .await?)
    }

    /// Return a stream of events for the given event filter.
    ///
    /// The ordering of the events can be set with the `descending_order`
    /// argument. For a list of possible event filters, see [EventFilter].
    pub fn get_events_stream(
        &self,
        query: EventFilter,
        cursor: Option<EventID>,
        descending_order: bool,
    ) -> impl Stream<Item = IotaEvent> + '_ {
        stream::unfold(
            (vec![], cursor, true, query),
            move |(mut data, cursor, first, query)| async move {
                if let Some(item) = data.pop() {
                    Some((item, (data, cursor, false, query)))
                } else if (cursor.is_none() && first) || cursor.is_some() {
                    let page = self
                        .query_events(query.clone(), cursor, Some(100), descending_order)
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
}

/// Quorum API that provides functionality to execute a transaction block and
/// submit it to the fullnode(s).
#[derive(Clone)]
pub struct QuorumDriverApi {
    api: Arc<RpcClient>,
}

impl QuorumDriverApi {
    pub(crate) fn new(api: Arc<RpcClient>) -> Self {
        Self { api }
    }

    /// Execute a transaction with a FullNode client. `request_type`
    /// defaults to `ExecuteTransactionRequestType::WaitForLocalExecution`.
    /// When `ExecuteTransactionRequestType::WaitForLocalExecution` is used,
    /// but returned `confirmed_local_execution` is false, the client will
    /// keep retry for WAIT_FOR_LOCAL_EXECUTION_RETRY_COUNT times. If it
    /// still fails, it will return an error.
    pub async fn execute_transaction_block(
        &self,
        tx: Transaction,
        options: IotaTransactionBlockResponseOptions,
        request_type: Option<ExecuteTransactionRequestType>,
    ) -> IotaRpcResult<IotaTransactionBlockResponse> {
        let (tx_bytes, signatures) = tx.to_tx_bytes_and_signatures();
        let request_type = request_type.unwrap_or_else(|| options.default_execution_request_type());

        let start = Instant::now();
        let response = self
            .api
            .http
            .execute_transaction_block(
                tx_bytes.clone(),
                signatures.clone(),
                Some(options.clone()),
                Some(request_type.clone()),
            )
            .await?;

        if let ExecuteTransactionRequestType::WaitForEffectsCert = request_type {
            return Ok(response);
        }

        // JSON-RPC ignores WaitForLocalExecution, so simulate it by polling for the
        // transaction.
        let mut poll_response = tokio::time::timeout(WAIT_FOR_LOCAL_EXECUTION_TIMEOUT, async {
            // Apply a short delay to give the full node a chance to catch up.
            tokio::time::sleep(WAIT_FOR_LOCAL_EXECUTION_DELAY).await;

            let mut interval = tokio::time::interval(WAIT_FOR_LOCAL_EXECUTION_INTERVAL);
            loop {
                interval.tick().await;

                if let Ok(poll_response) = self
                    .api
                    .http
                    .get_transaction_block(*tx.digest(), Some(options.clone()))
                    .await
                {
                    break poll_response;
                }
            }
        })
        .await
        .map_err(|_| {
            Error::FailToConfirmTransactionStatus(*tx.digest(), start.elapsed().as_secs())
        })?;

        poll_response.confirmed_local_execution = Some(true);
        Ok(poll_response)
    }
}

/// Governance API provides the staking functionality.
#[derive(Debug, Clone)]
pub struct GovernanceApi {
    api: Arc<RpcClient>,
}

impl GovernanceApi {
    pub(crate) fn new(api: Arc<RpcClient>) -> Self {
        Self { api }
    }

    /// Return a list of [DelegatedStake] objects for the given address, or an
    /// error upon failure.
    pub async fn get_stakes(&self, owner: IotaAddress) -> IotaRpcResult<Vec<DelegatedStake>> {
        Ok(self.api.http.get_stakes(owner).await?)
    }

    /// Return the [IotaCommittee] information for the given `epoch`, or an
    /// error upon failure.
    ///
    /// The argument `epoch` is the known epoch id or `None` for the current
    /// epoch.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use iota_sdk::IotaClientBuilder;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), anyhow::Error> {
    ///     let iota = IotaClientBuilder::default().build_localnet().await?;
    ///     let committee_info = iota.governance_api().get_committee_info(None).await?;
    ///     Ok(())
    /// }
    /// ```
    pub async fn get_committee_info(
        &self,
        epoch: Option<BigInt<u64>>,
    ) -> IotaRpcResult<IotaCommittee> {
        Ok(self.api.http.get_committee_info(epoch).await?)
    }

    /// Return the latest IOTA system state object on-chain, or an error upon
    /// failure.
    ///
    /// Use this method to access system's information, such as the current
    /// epoch, the protocol version, the reference gas price, the total
    /// stake, active validators, and much more. See the
    /// [IotaSystemStateSummary] for all the available fields.
    pub async fn get_latest_iota_system_state(&self) -> IotaRpcResult<IotaSystemStateSummary> {
        Ok(self.api.http.get_latest_iota_system_state().await?)
    }

    /// Return the reference gas price for the network, or an error upon
    /// failure.
    pub async fn get_reference_gas_price(&self) -> IotaRpcResult<u64> {
        Ok(*self.api.http.get_reference_gas_price().await?)
    }
}
