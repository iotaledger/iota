// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use cached::{SizedCache, proc_macro::cached};
use iota_core::authority::AuthorityState;
use iota_json_rpc_api::{CoinReadApiOpenRpc, CoinReadApiServer, JsonRpcMetrics, cap_page_limit};
use iota_json_rpc_types::{Balance, CoinPage, IotaCoinMetadata};
use iota_metrics::spawn_monitored_task;
use iota_open_rpc::Module;
use iota_storage::{indexes::TotalBalance, key_value_store::TransactionKeyValueStore};
use iota_types::{
    balance::Supply,
    base_types::{IotaAddress, ObjectID},
    coin::{CoinMetadata, TreasuryCap},
    effects::TransactionEffectsAPI,
    gas_coin::GAS,
    iota_system_state::IotaSystemStateTrait,
    object::Object,
    parse_iota_struct_tag,
};
use jsonrpsee::{RpcModule, core::RpcResult};
#[cfg(test)]
use mockall::automock;
use move_core_types::language_storage::{StructTag, TypeTag};
use tap::TapFallible;
use tracing::{debug, instrument};

use crate::{
    IotaRpcModule,
    authority_state::StateRead,
    error::{Error, IotaRpcInputError, RpcInterimResult},
    logger::FutureWithTracing as _,
};

pub fn parse_to_struct_tag(coin_type: &str) -> Result<StructTag, IotaRpcInputError> {
    parse_iota_struct_tag(coin_type)
        .map_err(|e| IotaRpcInputError::CannotParseIotaStructTag(format!("{e}")))
}

pub fn parse_to_type_tag(coin_type: Option<String>) -> Result<TypeTag, IotaRpcInputError> {
    Ok(TypeTag::Struct(Box::new(match coin_type {
        Some(c) => parse_to_struct_tag(&c)?,
        None => GAS::type_(),
    })))
}

pub struct CoinReadApi {
    // Trait object w/ Box as we do not need to share this across multiple threads
    internal: Box<dyn CoinReadInternal + Send + Sync>,
}

impl CoinReadApi {
    pub fn new(
        state: Arc<AuthorityState>,
        transaction_kv_store: Arc<TransactionKeyValueStore>,
        metrics: Arc<JsonRpcMetrics>,
    ) -> Self {
        Self {
            internal: Box::new(CoinReadInternalImpl::new(
                state,
                transaction_kv_store,
                metrics,
            )),
        }
    }
}

impl IotaRpcModule for CoinReadApi {
    fn rpc(self) -> RpcModule<Self> {
        self.into_rpc()
    }

    fn rpc_doc_module() -> Module {
        CoinReadApiOpenRpc::module_doc()
    }
}

#[async_trait]
impl CoinReadApiServer for CoinReadApi {
    #[instrument(skip(self))]
    async fn get_coins(
        &self,
        owner: IotaAddress,
        coin_type: Option<String>,
        // exclusive cursor if `Some`, otherwise start from the beginning
        cursor: Option<ObjectID>,
        limit: Option<usize>,
    ) -> RpcResult<CoinPage> {
        async move {
            let coin_type_tag = parse_to_type_tag(coin_type)?;

            let cursor = match cursor {
                Some(c) => (coin_type_tag.to_string(), c),
                // If cursor is not specified, we need to start from the beginning of the coin
                // type, which is the minimal possible ObjectID.
                None => (coin_type_tag.to_string(), ObjectID::ZERO),
            };

            self.internal
                .get_coins_iterator(
                    owner, cursor, limit, true, // only care about one type of coin
                )
                .await
        }
        .trace()
        .await
    }

    #[instrument(skip(self))]
    async fn get_all_coins(
        &self,
        owner: IotaAddress,
        // exclusive cursor if `Some`, otherwise start from the beginning
        cursor: Option<ObjectID>,
        limit: Option<usize>,
    ) -> RpcResult<CoinPage> {
        async move {
            let cursor = match cursor {
                Some(object_id) => {
                    let obj = self.internal.get_object(&object_id).await?;
                    match obj {
                        Some(obj) => {
                            let coin_type = obj.coin_type_maybe();
                            if coin_type.is_none() {
                                Err(IotaRpcInputError::GenericInvalid(
                                    "cursor is not a coin".to_string(),
                                ))
                            } else {
                                Ok((coin_type.unwrap().to_string(), object_id))
                            }
                        }
                        None => Err(IotaRpcInputError::GenericInvalid(
                            "cursor not found".to_string(),
                        )),
                    }
                }
                None => {
                    // If cursor is None, start from the beginning
                    Ok((String::from_utf8([0u8].to_vec()).unwrap(), ObjectID::ZERO))
                }
            }?;

            let coins = self
                .internal
                .get_coins_iterator(
                    owner, cursor, limit, false, // return all types of coins
                )
                .await?;

            Ok(coins)
        }
        .trace()
        .await
    }

    #[instrument(skip(self))]
    async fn get_balance(
        &self,
        owner: IotaAddress,
        coin_type: Option<String>,
    ) -> RpcResult<Balance> {
        async move {
            let coin_type_tag = parse_to_type_tag(coin_type)?;
            let balance = self
                .internal
                .get_balance(owner, coin_type_tag.clone())
                .await
                .tap_err(|e| {
                    debug!(?owner, "Failed to get balance with error: {:?}", e);
                })?;
            Ok(Balance {
                coin_type: coin_type_tag.to_string(),
                coin_object_count: balance.num_coins as usize,
                total_balance: balance.balance as u128,
            })
        }
        .trace()
        .await
    }

    #[instrument(skip(self))]
    async fn get_all_balances(&self, owner: IotaAddress) -> RpcResult<Vec<Balance>> {
        async move {
            let all_balance = self.internal.get_all_balance(owner).await.tap_err(|e| {
                debug!(?owner, "Failed to get all balance with error: {:?}", e);
            })?;
            Ok(all_balance
                .iter()
                .map(|(coin_type, balance)| Balance {
                    coin_type: coin_type.to_string(),
                    coin_object_count: balance.num_coins as usize,
                    total_balance: balance.balance as u128,
                })
                .collect())
        }
        .trace()
        .await
    }

    #[instrument(skip(self))]
    async fn get_coin_metadata(&self, coin_type: String) -> RpcResult<Option<IotaCoinMetadata>> {
        async move {
            let coin_struct = parse_to_struct_tag(&coin_type)?;
            let metadata_object = self
                .internal
                .find_package_object(
                    &coin_struct.address.into(),
                    CoinMetadata::type_(coin_struct),
                )
                .await
                .ok();
            Ok(metadata_object.and_then(|v: Object| v.try_into().ok()))
        }
        .trace()
        .await
    }

    #[instrument(skip(self))]
    async fn get_total_supply(&self, coin_type: String) -> RpcResult<Supply> {
        async move {
            let coin_struct = parse_to_struct_tag(&coin_type)?;
            Ok(if GAS::is_gas(&coin_struct) {
                let system_state_summary = self
                    .internal
                    .get_state()
                    .get_system_state()?
                    .into_iota_system_state_summary();
                Supply {
                    value: system_state_summary.iota_total_supply,
                }
            } else {
                let treasury_cap_object = self
                    .internal
                    .find_package_object(
                        &coin_struct.address.into(),
                        TreasuryCap::type_(coin_struct),
                    )
                    .await?;
                let treasury_cap = TreasuryCap::from_bcs_bytes(
                    treasury_cap_object.data.try_as_move().unwrap().contents(),
                )
                .map_err(Error::from)?;
                treasury_cap.total_supply
            })
        }
        .trace()
        .await
    }
}

#[cached(
    type = "SizedCache<String, ObjectID>",
    create = "{ SizedCache::with_size(10000) }",
    convert = r#"{ format!("{}{}", package_id, object_struct_tag) }"#,
    result = true
)]
async fn find_package_object_id(
    state: Arc<dyn StateRead>,
    package_id: ObjectID,
    object_struct_tag: StructTag,
    kv_store: Arc<TransactionKeyValueStore>,
) -> RpcInterimResult<ObjectID> {
    spawn_monitored_task!(async move {
        let publish_txn_digest = state.find_publish_txn_digest(package_id)?;

        let (_, effect) = state
            .get_executed_transaction_and_effects(publish_txn_digest, kv_store)
            .await?;

        for ((id, _, _), _) in effect.created() {
            if let Ok(object_read) = state.get_object_read(&id) {
                if let Ok(object) = object_read.into_object() {
                    if matches!(object.type_(), Some(type_) if type_.is(&object_struct_tag)) {
                        return Ok(id);
                    }
                }
            }
        }
        Err(IotaRpcInputError::GenericNotFound(format!(
            "Cannot find object [{}] from [{}] package event.",
            object_struct_tag, package_id,
        ))
        .into())
    })
    .await?
}

/// CoinReadInternal trait to capture logic of interactions with AuthorityState
/// and metrics This allows us to also mock internal implementation for testing
#[cfg_attr(test, automock)]
#[async_trait]
pub trait CoinReadInternal {
    fn get_state(&self) -> Arc<dyn StateRead>;
    async fn get_object(&self, object_id: &ObjectID) -> RpcInterimResult<Option<Object>>;
    async fn get_balance(
        &self,
        owner: IotaAddress,
        coin_type: TypeTag,
    ) -> RpcInterimResult<TotalBalance>;
    async fn get_all_balance(
        &self,
        owner: IotaAddress,
    ) -> RpcInterimResult<Arc<HashMap<TypeTag, TotalBalance>>>;
    async fn find_package_object(
        &self,
        package_id: &ObjectID,
        object_struct_tag: StructTag,
    ) -> RpcInterimResult<Object>;
    async fn get_coins_iterator(
        &self,
        owner: IotaAddress,
        cursor: (String, ObjectID),
        limit: Option<usize>,
        one_coin_type_only: bool,
    ) -> RpcInterimResult<CoinPage>;
}

pub struct CoinReadInternalImpl {
    // Trait object w/ Arc as we have methods that require sharing this across multiple threads
    state: Arc<dyn StateRead>,
    transaction_kv_store: Arc<TransactionKeyValueStore>,
    pub metrics: Arc<JsonRpcMetrics>,
}

impl CoinReadInternalImpl {
    pub fn new(
        state: Arc<AuthorityState>,
        transaction_kv_store: Arc<TransactionKeyValueStore>,
        metrics: Arc<JsonRpcMetrics>,
    ) -> Self {
        Self {
            state,
            transaction_kv_store,
            metrics,
        }
    }
}

#[async_trait]
impl CoinReadInternal for CoinReadInternalImpl {
    fn get_state(&self) -> Arc<dyn StateRead> {
        self.state.clone()
    }

    async fn get_object(&self, object_id: &ObjectID) -> RpcInterimResult<Option<Object>> {
        Ok(self.state.get_object(object_id).await?)
    }

    async fn get_balance(
        &self,
        owner: IotaAddress,
        coin_type: TypeTag,
    ) -> RpcInterimResult<TotalBalance> {
        Ok(self.state.get_balance(owner, coin_type).await?)
    }

    async fn get_all_balance(
        &self,
        owner: IotaAddress,
    ) -> RpcInterimResult<Arc<HashMap<TypeTag, TotalBalance>>> {
        Ok(self.state.get_all_balance(owner).await?)
    }

    async fn find_package_object(
        &self,
        package_id: &ObjectID,
        object_struct_tag: StructTag,
    ) -> RpcInterimResult<Object> {
        let state = self.get_state();
        let kv_store = self.transaction_kv_store.clone();
        let object_id =
            find_package_object_id(state, *package_id, object_struct_tag, kv_store).await?;
        Ok(self.state.get_object_read(&object_id)?.into_object()?)
    }

    async fn get_coins_iterator(
        &self,
        owner: IotaAddress,
        cursor: (String, ObjectID),
        limit: Option<usize>,
        one_coin_type_only: bool,
    ) -> RpcInterimResult<CoinPage> {
        let limit = cap_page_limit(limit);
        self.metrics.get_coins_limit.report(limit as u64);
        let state = self.get_state();
        let mut data = spawn_monitored_task!(async move {
            state.get_owned_coins(owner, cursor, limit + 1, one_coin_type_only)
        })
        .await??;

        let has_next_page = data.len() > limit;
        data.truncate(limit);

        self.metrics.get_coins_result_size.report(data.len() as u64);
        self.metrics
            .get_coins_result_size_total
            .inc_by(data.len() as u64);
        let next_cursor = data.last().map(|coin| coin.coin_object_id);
        Ok(CoinPage {
            data,
            next_cursor,
            has_next_page,
        })
    }
}

#[cfg(test)]
mod tests {
    use expect_test::expect;
    use iota_json_rpc_types::Coin;
    use iota_storage::{
        key_value_store::{
            KVStoreCheckpointData, KVStoreTransactionData, TransactionKeyValueStoreTrait,
        },
        key_value_store_metrics::KeyValueStoreMetrics,
    };
    use iota_types::{
        TypeTag,
        balance::Supply,
        base_types::{IotaAddress, ObjectID, SequenceNumber},
        coin::TreasuryCap,
        digests::{ObjectDigest, TransactionDigest, TransactionEventsDigest},
        effects::{TransactionEffects, TransactionEvents},
        error::{IotaError, IotaResult},
        gas_coin::GAS,
        id::UID,
        messages_checkpoint::{CheckpointDigest, CheckpointSequenceNumber},
        object::Object,
        parse_iota_struct_tag,
        utils::create_fake_transaction,
    };
    use mockall::{mock, predicate};
    use move_core_types::{account_address::AccountAddress, language_storage::StructTag};

    use super::*;
    use crate::authority_state::{MockStateRead, StateReadError};

    mock! {
        pub KeyValueStore {}
        #[async_trait]
        impl TransactionKeyValueStoreTrait for KeyValueStore {
            async fn multi_get(
                &self,
                transactions: &[TransactionDigest],
                effects: &[TransactionDigest],
                events: &[TransactionEventsDigest],
            ) -> IotaResult<KVStoreTransactionData>;

            async fn multi_get_checkpoints(
                &self,
                checkpoint_summaries: &[CheckpointSequenceNumber],
                checkpoint_contents: &[CheckpointSequenceNumber],
                checkpoint_summaries_by_digest: &[CheckpointDigest],
            ) -> IotaResult<KVStoreCheckpointData>;

            async fn get_transaction_perpetual_checkpoint(
                &self,
                digest: TransactionDigest,
            ) -> IotaResult<Option<CheckpointSequenceNumber>>;

            async fn get_object(&self, object_id: ObjectID, version: SequenceNumber) -> IotaResult<Option<Object>>;

            async fn multi_get_transactions_perpetual_checkpoints(
                &self,
                digests: &[TransactionDigest],
            ) -> IotaResult<Vec<Option<CheckpointSequenceNumber>>>;

            async fn multi_get_events_by_tx_digests(
                &self,
                digests: &[TransactionDigest]
            ) -> IotaResult<Vec<Option<TransactionEvents>>>;
        }
    }

    impl CoinReadInternalImpl {
        pub fn new_for_tests(
            state: Arc<MockStateRead>,
            kv_store: Option<Arc<MockKeyValueStore>>,
        ) -> Self {
            let kv_store = kv_store.unwrap_or_else(|| Arc::new(MockKeyValueStore::new()));
            let metrics = KeyValueStoreMetrics::new_for_tests();
            let transaction_kv_store =
                Arc::new(TransactionKeyValueStore::new("rocksdb", metrics, kv_store));
            Self {
                state,
                transaction_kv_store,
                metrics: Arc::new(JsonRpcMetrics::new_for_tests()),
            }
        }
    }

    impl CoinReadApi {
        pub fn new_for_tests(
            state: Arc<MockStateRead>,
            kv_store: Option<Arc<MockKeyValueStore>>,
        ) -> Self {
            let kv_store = kv_store.unwrap_or_else(|| Arc::new(MockKeyValueStore::new()));
            Self {
                internal: Box::new(CoinReadInternalImpl::new_for_tests(state, Some(kv_store))),
            }
        }
    }

    fn get_test_owner() -> IotaAddress {
        AccountAddress::ONE.into()
    }

    fn get_test_package_id() -> ObjectID {
        ObjectID::from_hex_literal("0xf").unwrap()
    }

    fn get_test_coin_type(package_id: ObjectID) -> String {
        format!("{}::test_coin::TEST_COIN", package_id)
    }

    fn get_test_coin_type_tag(coin_type: String) -> TypeTag {
        TypeTag::Struct(Box::new(parse_iota_struct_tag(&coin_type).unwrap()))
    }

    enum CoinType {
        Gas,
        Usdc,
    }

    fn get_test_coin(id_hex_literal: Option<&str>, coin_type: CoinType) -> Coin {
        let (arr, coin_type_string, balance, default_hex) = match coin_type {
            CoinType::Gas => ([0; 32], GAS::type_().to_string(), 42, "0xA"),
            CoinType::Usdc => (
                [1; 32],
                "0x168da5bf1f48dafc111b0a488fa454aca95e0b5e::usdc::USDC".to_string(),
                24,
                "0xB",
            ),
        };

        let object_id = if let Some(literal) = id_hex_literal {
            ObjectID::from_hex_literal(literal).unwrap()
        } else {
            ObjectID::from_hex_literal(default_hex).unwrap()
        };

        Coin {
            coin_type: coin_type_string,
            coin_object_id: object_id,
            version: SequenceNumber::from_u64(1),
            digest: ObjectDigest::from(arr),
            balance,
            previous_transaction: TransactionDigest::from(arr),
        }
    }

    fn get_test_treasury_cap_peripherals(
        package_id: ObjectID,
    ) -> (String, StructTag, StructTag, TreasuryCap, Object) {
        let coin_name = get_test_coin_type(package_id);
        let input_coin_struct = parse_iota_struct_tag(&coin_name).expect("should not fail");
        let treasury_cap_struct = TreasuryCap::type_(input_coin_struct.clone());
        let treasury_cap = TreasuryCap {
            id: UID::new(get_test_package_id()),
            total_supply: Supply { value: 420 },
        };
        let treasury_cap_object =
            Object::treasury_cap_for_testing(input_coin_struct.clone(), treasury_cap.clone());
        (
            coin_name,
            input_coin_struct,
            treasury_cap_struct,
            treasury_cap,
            treasury_cap_object,
        )
    }

    mod get_coins_tests {
        use super::{super::*, *};

        // Success scenarios
        #[tokio::test]
        async fn test_gas_coin_no_cursor() {
            let owner = get_test_owner();
            let gas_coin = get_test_coin(None, CoinType::Gas);
            let gas_coin_clone = gas_coin.clone();
            let mut mock_state = MockStateRead::new();
            mock_state
                .expect_get_owned_coins()
                .with(
                    predicate::eq(owner),
                    predicate::eq((GAS::type_().to_string(), ObjectID::ZERO)),
                    predicate::eq(51),
                    predicate::eq(true),
                )
                .return_once(move |_, _, _, _| Ok(vec![gas_coin_clone]));

            let coin_read_api = CoinReadApi::new_for_tests(Arc::new(mock_state), None);
            let response = coin_read_api.get_coins(owner, None, None, None).await;
            assert!(response.is_ok());
            let result = response.unwrap();
            assert_eq!(
                result,
                CoinPage {
                    data: vec![gas_coin.clone()],
                    next_cursor: Some(gas_coin.coin_object_id),
                    has_next_page: false,
                }
            );
        }

        #[tokio::test]
        async fn test_gas_coin_with_cursor() {
            let owner = get_test_owner();
            let limit = 2;
            let coins = vec![
                get_test_coin(Some("0xA"), CoinType::Gas),
                get_test_coin(Some("0xAA"), CoinType::Gas),
                get_test_coin(Some("0xAAA"), CoinType::Gas),
            ];
            let coins_clone = coins.clone();
            let mut mock_state = MockStateRead::new();
            mock_state
                .expect_get_owned_coins()
                .with(
                    predicate::eq(owner),
                    predicate::eq((GAS::type_().to_string(), coins[0].coin_object_id)),
                    predicate::eq(limit + 1),
                    predicate::eq(true),
                )
                .return_once(move |_, _, _, _| Ok(coins_clone));

            let coin_read_api = CoinReadApi::new_for_tests(Arc::new(mock_state), None);
            let response = coin_read_api
                .get_coins(owner, None, Some(coins[0].coin_object_id), Some(limit))
                .await;
            assert!(response.is_ok());
            let result = response.unwrap();
            assert_eq!(
                result,
                CoinPage {
                    data: coins[..limit].to_vec(),
                    next_cursor: Some(coins[limit - 1].coin_object_id),
                    has_next_page: true,
                }
            );
        }

        #[tokio::test]
        async fn test_coin_no_cursor() {
            let coin = get_test_coin(None, CoinType::Usdc);
            let coin_clone = coin.clone();
            // Build request params
            let owner = get_test_owner();
            let coin_type = coin.coin_type.clone();

            let coin_type_tag =
                TypeTag::Struct(Box::new(parse_iota_struct_tag(&coin.coin_type).unwrap()));
            let mut mock_state = MockStateRead::new();
            mock_state
                .expect_get_owned_coins()
                .with(
                    predicate::eq(owner),
                    predicate::eq((coin_type_tag.to_string(), ObjectID::ZERO)),
                    predicate::eq(51),
                    predicate::eq(true),
                )
                .return_once(move |_, _, _, _| Ok(vec![coin_clone]));

            let coin_read_api = CoinReadApi::new_for_tests(Arc::new(mock_state), None);
            let response = coin_read_api
                .get_coins(owner, Some(coin_type), None, None)
                .await;

            assert!(response.is_ok());
            let result = response.unwrap();
            assert_eq!(
                result,
                CoinPage {
                    data: vec![coin.clone()],
                    next_cursor: Some(coin.coin_object_id),
                    has_next_page: false,
                }
            );
        }

        #[tokio::test]
        async fn test_coin_with_cursor() {
            let coins = vec![
                get_test_coin(Some("0xB"), CoinType::Usdc),
                get_test_coin(Some("0xBB"), CoinType::Usdc),
                get_test_coin(Some("0xBBB"), CoinType::Usdc),
            ];
            let coins_clone = coins.clone();
            // Build request params
            let owner = get_test_owner();
            let coin_type = coins[0].coin_type.clone();
            let cursor = coins[0].coin_object_id;
            let limit = 2;

            let coin_type_tag = TypeTag::Struct(Box::new(
                parse_iota_struct_tag(&coins[0].coin_type).unwrap(),
            ));
            let mut mock_state = MockStateRead::new();
            mock_state
                .expect_get_owned_coins()
                .with(
                    predicate::eq(owner),
                    predicate::eq((coin_type_tag.to_string(), coins[0].coin_object_id)),
                    predicate::eq(limit + 1),
                    predicate::eq(true),
                )
                .return_once(move |_, _, _, _| Ok(coins_clone));

            let coin_read_api = CoinReadApi::new_for_tests(Arc::new(mock_state), None);
            let response = coin_read_api
                .get_coins(owner, Some(coin_type), Some(cursor), Some(limit))
                .await;

            assert!(response.is_ok());
            let result = response.unwrap();
            assert_eq!(
                result,
                CoinPage {
                    data: coins[..limit].to_vec(),
                    next_cursor: Some(coins[limit - 1].coin_object_id),
                    has_next_page: true,
                }
            );
        }

        // Expected error scenarios
        #[tokio::test]
        async fn test_invalid_coin_type() {
            let owner = get_test_owner();
            let coin_type = "0x2::invalid::struct::tag";
            let mock_state = MockStateRead::new();
            let coin_read_api = CoinReadApi::new_for_tests(Arc::new(mock_state), None);
            let response = coin_read_api
                .get_coins(owner, Some(coin_type.to_string()), None, None)
                .await;

            assert!(response.is_err());
            let error_result = response.unwrap_err();
            let expected = expect!["-32602"];
            expected.assert_eq(&error_result.code().to_string());
            let expected = expect![
                "Invalid struct type: 0x2::invalid::struct::tag. Got error: Expected end of token stream. Got: ::"
            ];
            expected.assert_eq(error_result.message());
        }

        #[tokio::test]
        async fn test_unrecognized_token() {
            let owner = get_test_owner();
            let coin_type = "0x2::iota:🤵";
            let mock_state = MockStateRead::new();
            let coin_read_api = CoinReadApi::new_for_tests(Arc::new(mock_state), None);
            let response = coin_read_api
                .get_coins(owner, Some(coin_type.to_string()), None, None)
                .await;

            assert!(response.is_err());
            let error_result = response.unwrap_err();
            let expected = expect!["-32602"];
            expected.assert_eq(&error_result.code().to_string());
            let expected =
                expect!["Invalid struct type: 0x2::iota:🤵. Got error: unrecognized token: :🤵"];
            expected.assert_eq(error_result.message());
        }

        // Unexpected error scenarios
        #[tokio::test]
        async fn test_get_coins_iterator_index_store_not_available() {
            let owner = get_test_owner();
            let coin_type = get_test_coin_type(get_test_package_id());
            let mut mock_state = MockStateRead::new();
            mock_state
                .expect_get_owned_coins()
                .returning(move |_, _, _, _| {
                    Err(StateReadError::Client(
                        IotaError::IndexStoreNotAvailable.into(),
                    ))
                });
            let coin_read_api = CoinReadApi::new_for_tests(Arc::new(mock_state), None);
            let response = coin_read_api
                .get_coins(owner, Some(coin_type.to_string()), None, None)
                .await;

            assert!(response.is_err());
            let error_result = response.unwrap_err();
            assert_eq!(
                error_result.code(),
                jsonrpsee::types::error::INVALID_PARAMS_CODE
            );
            let expected = expect!["Index store not available on this Fullnode."];
            expected.assert_eq(error_result.message());
        }

        #[tokio::test]
        async fn test_get_coins_iterator_typed_store_error() {
            let owner = get_test_owner();
            let coin_type = get_test_coin_type(get_test_package_id());
            let mut mock_state = MockStateRead::new();
            mock_state
                .expect_get_owned_coins()
                .returning(move |_, _, _, _| {
                    Err(IotaError::Storage("mock rocksdb error".to_string()).into())
                });
            let coin_read_api = CoinReadApi::new_for_tests(Arc::new(mock_state), None);
            let response = coin_read_api
                .get_coins(owner, Some(coin_type.to_string()), None, None)
                .await;

            assert!(response.is_err());
            let error_result = response.unwrap_err();
            assert_eq!(
                error_result.code(),
                jsonrpsee::types::error::INTERNAL_ERROR_CODE
            );
            let expected = expect!["Storage error: mock rocksdb error"];
            expected.assert_eq(error_result.message());
        }
    }

    mod get_all_coins_tests {
        use iota_types::object::{MoveObject, Owner};

        use super::{super::*, *};

        // Success scenarios
        #[tokio::test]
        async fn test_no_cursor() {
            let owner = get_test_owner();
            let gas_coin = get_test_coin(None, CoinType::Gas);
            let gas_coin_clone = gas_coin.clone();
            let mut mock_state = MockStateRead::new();
            mock_state
                .expect_get_owned_coins()
                .with(
                    predicate::eq(owner),
                    predicate::eq((String::from_utf8([0u8].to_vec()).unwrap(), ObjectID::ZERO)),
                    predicate::eq(51),
                    predicate::eq(false),
                )
                .return_once(move |_, _, _, _| Ok(vec![gas_coin_clone]));
            let coin_read_api = CoinReadApi::new_for_tests(Arc::new(mock_state), None);
            let response = coin_read_api
                .get_all_coins(owner, None, Some(51))
                .await
                .unwrap();
            assert_eq!(response.data.len(), 1);
            assert_eq!(response.data[0], gas_coin);
        }

        #[tokio::test]
        async fn test_with_cursor() {
            let owner = get_test_owner();
            let limit = 2;
            let coins = vec![
                get_test_coin(Some("0xA"), CoinType::Gas),
                get_test_coin(Some("0xAA"), CoinType::Gas),
                get_test_coin(Some("0xAAA"), CoinType::Gas),
            ];
            let coins_clone = coins.clone();
            let coin_move_object = MoveObject::new_gas_coin(
                coins[0].version,
                coins[0].coin_object_id,
                coins[0].balance,
            );
            let coin_object = Object::new_move(
                coin_move_object,
                Owner::Immutable,
                coins[0].previous_transaction,
            );
            let mut mock_state = MockStateRead::new();
            mock_state
                .expect_get_object()
                .return_once(move |_| Ok(Some(coin_object)));
            mock_state
                .expect_get_owned_coins()
                .with(
                    predicate::eq(owner),
                    predicate::eq((coins[0].coin_type.clone(), coins[0].coin_object_id)),
                    predicate::eq(limit + 1),
                    predicate::eq(false),
                )
                .return_once(move |_, _, _, _| Ok(coins_clone));
            let coin_read_api = CoinReadApi::new_for_tests(Arc::new(mock_state), None);
            let response = coin_read_api
                .get_all_coins(owner, Some(coins[0].coin_object_id), Some(limit))
                .await
                .unwrap();
            assert_eq!(response.data.len(), limit);
            assert_eq!(response.data, coins[..limit].to_vec());
        }

        // Expected error scenarios
        #[tokio::test]
        async fn test_object_is_not_coin() {
            let owner = get_test_owner();
            let object_id = get_test_package_id();
            let (_, _, _, _, treasury_cap_object) = get_test_treasury_cap_peripherals(object_id);
            let mut mock_state = MockStateRead::new();
            mock_state.expect_get_object().returning(move |obj_id| {
                if obj_id == &object_id {
                    Ok(Some(treasury_cap_object.clone()))
                } else {
                    panic!("should not be called with any other object id")
                }
            });
            let coin_read_api = CoinReadApi::new_for_tests(Arc::new(mock_state), None);
            let response = coin_read_api
                .get_all_coins(owner, Some(object_id), None)
                .await;

            assert!(response.is_err());
            let error_result = response.unwrap_err();
            assert_eq!(error_result.code(), -32602);
            let expected = expect!["-32602"];
            expected.assert_eq(&error_result.code().to_string());
            let expected = expect!["cursor is not a coin"];
            expected.assert_eq(error_result.message());
        }

        #[tokio::test]
        async fn test_object_not_found() {
            let owner = get_test_owner();
            let object_id = get_test_package_id();
            let mut mock_state = MockStateRead::new();
            mock_state.expect_get_object().returning(move |_| Ok(None));

            let coin_read_api = CoinReadApi::new_for_tests(Arc::new(mock_state), None);
            let response = coin_read_api
                .get_all_coins(owner, Some(object_id), None)
                .await;

            assert!(response.is_err());
            let error_result = response.unwrap_err();
            let expected = expect!["-32602"];
            expected.assert_eq(&error_result.code().to_string());
            let expected = expect!["cursor not found"];
            expected.assert_eq(error_result.message());
        }
    }

    mod get_balance_tests {

        use super::{super::*, *};
        // Success scenarios
        #[tokio::test]
        async fn test_gas_coin() {
            let owner = get_test_owner();
            let gas_coin = get_test_coin(None, CoinType::Gas);
            let gas_coin_clone = gas_coin.clone();
            let mut mock_state = MockStateRead::new();
            mock_state
                .expect_get_balance()
                .with(
                    predicate::eq(owner),
                    predicate::eq(get_test_coin_type_tag(gas_coin_clone.coin_type)),
                )
                .return_once(move |_, _| {
                    Ok(TotalBalance {
                        balance: 7,
                        num_coins: 9,
                    })
                });
            let coin_read_api = CoinReadApi::new_for_tests(Arc::new(mock_state), None);
            let response = coin_read_api.get_balance(owner, None).await;

            assert!(response.is_ok());
            let result = response.unwrap();
            assert_eq!(
                result,
                Balance {
                    coin_type: gas_coin.coin_type,
                    coin_object_count: 9,
                    total_balance: 7,
                }
            );
        }

        #[tokio::test]
        async fn test_with_coin_type() {
            let owner = get_test_owner();
            let coin = get_test_coin(None, CoinType::Usdc);
            let coin_clone = coin.clone();
            let mut mock_state = MockStateRead::new();
            mock_state
                .expect_get_balance()
                .with(
                    predicate::eq(owner),
                    predicate::eq(get_test_coin_type_tag(coin_clone.coin_type)),
                )
                .return_once(move |_, _| {
                    Ok(TotalBalance {
                        balance: 10,
                        num_coins: 11,
                    })
                });
            let coin_read_api = CoinReadApi::new_for_tests(Arc::new(mock_state), None);
            let response = coin_read_api
                .get_balance(owner, Some(coin.coin_type.clone()))
                .await;

            assert!(response.is_ok());
            let result = response.unwrap();
            assert_eq!(
                result,
                Balance {
                    coin_type: coin.coin_type,
                    coin_object_count: 11,
                    total_balance: 10,
                }
            );
        }

        // Expected error scenarios
        #[tokio::test]
        async fn test_invalid_coin_type() {
            let owner = get_test_owner();
            let coin_type = "0x2::invalid::struct::tag";
            let mock_state = MockStateRead::new();
            let coin_read_api = CoinReadApi::new_for_tests(Arc::new(mock_state), None);
            let response = coin_read_api
                .get_balance(owner, Some(coin_type.to_string()))
                .await;

            assert!(response.is_err());
            let error_result = response.unwrap_err();
            let expected = expect!["-32602"];
            expected.assert_eq(&error_result.code().to_string());
            let expected = expect![
                "Invalid struct type: 0x2::invalid::struct::tag. Got error: Expected end of token stream. Got: ::"
            ];
            expected.assert_eq(error_result.message());
        }

        // Unexpected error scenarios
        #[tokio::test]
        async fn test_get_balance_index_store_not_available() {
            let owner = get_test_owner();
            let coin_type = get_test_coin_type(get_test_package_id());
            let mut mock_state = MockStateRead::new();
            mock_state.expect_get_balance().returning(move |_, _| {
                Err(StateReadError::Client(
                    IotaError::IndexStoreNotAvailable.into(),
                ))
            });
            let coin_read_api = CoinReadApi::new_for_tests(Arc::new(mock_state), None);
            let response = coin_read_api
                .get_balance(owner, Some(coin_type.to_string()))
                .await;

            assert!(response.is_err());
            let error_result = response.unwrap_err();
            assert_eq!(
                error_result.code(),
                jsonrpsee::types::error::INVALID_PARAMS_CODE
            );
            let expected = expect!["Index store not available on this Fullnode."];
            expected.assert_eq(error_result.message());
        }

        #[tokio::test]
        async fn test_get_balance_execution_error() {
            // Validate that we handle and return an error message when we encounter an
            // unexpected error
            let owner = get_test_owner();
            let coin_type = get_test_coin_type(get_test_package_id());
            let mut mock_state = MockStateRead::new();
            mock_state.expect_get_balance().returning(move |_, _| {
                Err(IotaError::Execution("mock db error".to_string()).into())
            });
            let coin_read_api = CoinReadApi::new_for_tests(Arc::new(mock_state), None);
            let response = coin_read_api
                .get_balance(owner, Some(coin_type.to_string()))
                .await;

            assert!(response.is_err());
            let error_result = response.unwrap_err();

            assert_eq!(
                error_result.code(),
                jsonrpsee::types::error::INTERNAL_ERROR_CODE
            );
            let expected = expect!["Error executing mock db error"];
            expected.assert_eq(error_result.message());
        }
    }

    mod get_all_balances_tests {
        use super::{super::*, *};

        // Success scenarios
        #[tokio::test]
        async fn test_success_scenario() {
            let owner = get_test_owner();
            let gas_coin = get_test_coin(None, CoinType::Gas);
            let gas_coin_type_tag = get_test_coin_type_tag(gas_coin.coin_type.clone());
            let usdc_coin = get_test_coin(None, CoinType::Usdc);
            let usdc_coin_type_tag = get_test_coin_type_tag(usdc_coin.coin_type.clone());
            let mut mock_state = MockStateRead::new();
            mock_state
                .expect_get_all_balance()
                .with(predicate::eq(owner))
                .return_once(move |_| {
                    let mut hash_map = HashMap::new();
                    hash_map.insert(
                        gas_coin_type_tag,
                        TotalBalance {
                            balance: 7,
                            num_coins: 9,
                        },
                    );
                    hash_map.insert(
                        usdc_coin_type_tag,
                        TotalBalance {
                            balance: 10,
                            num_coins: 11,
                        },
                    );
                    Ok(Arc::new(hash_map))
                });
            let coin_read_api = CoinReadApi::new_for_tests(Arc::new(mock_state), None);
            let response = coin_read_api.get_all_balances(owner).await;

            assert!(response.is_ok());
            let expected_result = vec![
                Balance {
                    coin_type: gas_coin.coin_type,
                    coin_object_count: 9,
                    total_balance: 7,
                },
                Balance {
                    coin_type: usdc_coin.coin_type,
                    coin_object_count: 11,
                    total_balance: 10,
                },
            ];
            // This is because the underlying result is a hashmap, so order is not
            // guaranteed
            let mut result = response.unwrap();
            for item in expected_result {
                if let Some(pos) = result.iter().position(|i| *i == item) {
                    result.remove(pos);
                } else {
                    panic!("{:?} not found in result", item);
                }
            }
            assert!(result.is_empty());
        }

        // Unexpected error scenarios
        #[tokio::test]
        async fn test_index_store_not_available() {
            let owner = get_test_owner();
            let mut mock_state = MockStateRead::new();
            mock_state.expect_get_all_balance().returning(move |_| {
                Err(StateReadError::Client(
                    IotaError::IndexStoreNotAvailable.into(),
                ))
            });
            let coin_read_api = CoinReadApi::new_for_tests(Arc::new(mock_state), None);
            let response = coin_read_api.get_all_balances(owner).await;

            assert!(response.is_err());
            let error_result = response.unwrap_err();
            assert_eq!(
                error_result.code(),
                jsonrpsee::types::error::INVALID_PARAMS_CODE
            );
            let expected = expect!["Index store not available on this Fullnode."];
            expected.assert_eq(error_result.message());
        }
    }

    mod get_coin_metadata_tests {
        use iota_types::id::UID;
        use mockall::predicate;

        use super::{super::*, *};

        // Success scenarios
        #[tokio::test]
        async fn test_valid_coin_metadata_object() {
            let package_id = get_test_package_id();
            let coin_name = get_test_coin_type(package_id);
            let input_coin_struct = parse_iota_struct_tag(&coin_name).expect("should not fail");
            let coin_metadata_struct = CoinMetadata::type_(input_coin_struct.clone());
            let coin_metadata = CoinMetadata {
                id: UID::new(get_test_package_id()),
                decimals: 2,
                name: "test_coin".to_string(),
                symbol: "TEST".to_string(),
                description: "test coin".to_string(),
                icon_url: Some("unit.test.io".to_string()),
            };
            let coin_metadata_object =
                Object::coin_metadata_for_testing(input_coin_struct.clone(), coin_metadata);
            let metadata = IotaCoinMetadata::try_from(coin_metadata_object.clone()).unwrap();
            let mut mock_internal = MockCoinReadInternal::new();
            // return TreasuryCap instead of CoinMetadata to set up test
            mock_internal
                .expect_find_package_object()
                .with(predicate::always(), predicate::eq(coin_metadata_struct))
                .return_once(move |object_id, _| {
                    if object_id == &package_id {
                        Ok(coin_metadata_object)
                    } else {
                        panic!("should not be called with any other object id")
                    }
                });

            let coin_read_api = CoinReadApi {
                internal: Box::new(mock_internal),
            };

            let response = coin_read_api.get_coin_metadata(coin_name.clone()).await;
            assert!(response.is_ok());
            let result = response.unwrap().unwrap();
            assert_eq!(result, metadata);
        }

        #[tokio::test]
        async fn test_object_not_found() {
            let transaction_digest = TransactionDigest::from([0; 32]);
            let transaction_effects = TransactionEffects::default();

            let mut mock_state = MockStateRead::new();
            mock_state
                .expect_find_publish_txn_digest()
                .return_once(move |_| Ok(transaction_digest));
            mock_state
                .expect_get_executed_transaction_and_effects()
                .return_once(move |_, _| Ok((create_fake_transaction(), transaction_effects)));

            let coin_read_api = CoinReadApi::new_for_tests(Arc::new(mock_state), None);
            let response = coin_read_api
                .get_coin_metadata("0x2::iota::IOTA".to_string())
                .await;

            assert!(response.is_ok());
            let result = response.unwrap();
            assert_eq!(result, None);
        }

        #[tokio::test]
        async fn test_find_package_object_not_iota_coin_metadata() {
            let package_id = get_test_package_id();
            let coin_name = get_test_coin_type(package_id);
            let input_coin_struct = parse_iota_struct_tag(&coin_name).expect("should not fail");
            let coin_metadata_struct = CoinMetadata::type_(input_coin_struct.clone());
            let treasury_cap = TreasuryCap {
                id: UID::new(get_test_package_id()),
                total_supply: Supply { value: 420 },
            };
            let treasury_cap_object =
                Object::treasury_cap_for_testing(input_coin_struct.clone(), treasury_cap);
            let mut mock_internal = MockCoinReadInternal::new();
            // return TreasuryCap instead of CoinMetadata to set up test
            mock_internal
                .expect_find_package_object()
                .with(predicate::always(), predicate::eq(coin_metadata_struct))
                .returning(move |object_id, _| {
                    if object_id == &package_id {
                        Ok(treasury_cap_object.clone())
                    } else {
                        panic!("should not be called with any other object id")
                    }
                });

            let coin_read_api = CoinReadApi {
                internal: Box::new(mock_internal),
            };

            let response = coin_read_api.get_coin_metadata(coin_name.clone()).await;
            assert!(response.is_ok());
            let result = response.unwrap();
            assert!(result.is_none());
        }
    }

    mod get_total_supply_tests {
        use iota_types::{
            collection_types::VecMap,
            gas_coin::IotaTreasuryCap,
            id::UID,
            iota_system_state::{
                IotaSystemState,
                iota_system_state_inner_v1::{
                    IotaSystemStateV1, StorageFundV1, SystemParametersV1, ValidatorSetV1,
                },
            },
        };
        use mockall::predicate;

        use super::{super::*, *};

        #[tokio::test]
        async fn test_success_response_for_gas_coin() {
            let coin_type = "0x2::iota::IOTA";

            let mut mock_state = MockStateRead::new();
            mock_state.expect_get_system_state().returning(move || {
                let mut state = default_system_state();
                state.iota_treasury_cap.inner.total_supply.value = 42;

                Ok(IotaSystemState::V1(state))
            });

            let coin_read_api = CoinReadApi::new_for_tests(Arc::new(mock_state), None);

            let response = coin_read_api.get_total_supply(coin_type.to_string()).await;

            let supply = response.unwrap();
            assert_eq!(supply.value, 42);
        }

        #[tokio::test]
        async fn test_success_response_for_other_coin() {
            let package_id = get_test_package_id();
            let (coin_name, _, treasury_cap_struct, _, treasury_cap_object) =
                get_test_treasury_cap_peripherals(package_id);
            let mut mock_internal = MockCoinReadInternal::new();
            mock_internal
                .expect_find_package_object()
                .with(predicate::always(), predicate::eq(treasury_cap_struct))
                .returning(move |object_id, _| {
                    if object_id == &package_id {
                        Ok(treasury_cap_object.clone())
                    } else {
                        panic!("should not be called with any other object id")
                    }
                });
            let coin_read_api = CoinReadApi {
                internal: Box::new(mock_internal),
            };

            let response = coin_read_api.get_total_supply(coin_name.clone()).await;

            assert!(response.is_ok());
            let result = response.unwrap();
            let expected = expect!["420"];
            expected.assert_eq(&result.value.to_string());
        }

        #[tokio::test]
        async fn test_object_not_found() {
            let package_id = get_test_package_id();
            let (coin_name, _, _, _, _) = get_test_treasury_cap_peripherals(package_id);
            let transaction_digest = TransactionDigest::from([0; 32]);
            let transaction_effects = TransactionEffects::default();

            let mut mock_state = MockStateRead::new();
            mock_state
                .expect_find_publish_txn_digest()
                .return_once(move |_| Ok(transaction_digest));
            mock_state
                .expect_get_executed_transaction_and_effects()
                .return_once(move |_, _| Ok((create_fake_transaction(), transaction_effects)));

            let coin_read_api = CoinReadApi::new_for_tests(Arc::new(mock_state), None);
            let response = coin_read_api.get_total_supply(coin_name.clone()).await;

            assert!(response.is_err());
            let error_result = response.unwrap_err();
            let expected = expect!["-32602"];
            expected.assert_eq(&error_result.code().to_string());
            let expected = expect![
                "Cannot find object [0x2::coin::TreasuryCap<0xf::test_coin::TEST_COIN>] from [0x000000000000000000000000000000000000000000000000000000000000000f] package event."
            ];
            expected.assert_eq(error_result.message());
        }

        #[tokio::test]
        async fn test_find_package_object_not_treasury_cap() {
            let package_id = get_test_package_id();
            let (coin_name, input_coin_struct, treasury_cap_struct, _, _) =
                get_test_treasury_cap_peripherals(package_id);
            let coin_metadata = CoinMetadata {
                id: UID::new(get_test_package_id()),
                decimals: 2,
                name: "test_coin".to_string(),
                symbol: "TEST".to_string(),
                description: "test coin".to_string(),
                icon_url: None,
            };
            let coin_metadata_object =
                Object::coin_metadata_for_testing(input_coin_struct.clone(), coin_metadata);
            let mut mock_internal = MockCoinReadInternal::new();
            mock_internal
                .expect_find_package_object()
                .with(predicate::always(), predicate::eq(treasury_cap_struct))
                .returning(move |object_id, _| {
                    if object_id == &package_id {
                        Ok(coin_metadata_object.clone())
                    } else {
                        panic!("should not be called with any other object id")
                    }
                });

            let coin_read_api = CoinReadApi {
                internal: Box::new(mock_internal),
            };

            let response = coin_read_api.get_total_supply(coin_name.clone()).await;
            let error_result = response.unwrap_err();
            assert_eq!(
                error_result.code(),
                jsonrpsee::types::error::CALL_EXECUTION_FAILED_CODE
            );
            let expected = expect![
                "Failure deserializing object in the requested format: \"Unable to deserialize TreasuryCap object: remaining input\""
            ];
            expected.assert_eq(error_result.message());
        }

        fn default_system_state() -> IotaSystemStateV1 {
            IotaSystemStateV1 {
                epoch: Default::default(),
                protocol_version: Default::default(),
                system_state_version: Default::default(),
                iota_treasury_cap: IotaTreasuryCap {
                    inner: TreasuryCap {
                        id: UID::new(ObjectID::random()),
                        total_supply: Supply {
                            value: Default::default(),
                        },
                    },
                },
                validators: ValidatorSetV1 {
                    total_stake: Default::default(),
                    active_validators: Default::default(),
                    pending_active_validators: Default::default(),
                    pending_removals: Default::default(),
                    staking_pool_mappings: Default::default(),
                    inactive_validators: Default::default(),
                    validator_candidates: Default::default(),
                    at_risk_validators: VecMap {
                        contents: Default::default(),
                    },
                    extra_fields: Default::default(),
                },
                storage_fund: StorageFundV1 {
                    total_object_storage_rebates: iota_types::balance::Balance::new(
                        Default::default(),
                    ),
                    non_refundable_balance: iota_types::balance::Balance::new(Default::default()),
                },
                parameters: SystemParametersV1 {
                    epoch_duration_ms: Default::default(),
                    min_validator_count: Default::default(),
                    max_validator_count: Default::default(),
                    min_validator_joining_stake: Default::default(),
                    validator_low_stake_threshold: Default::default(),
                    validator_very_low_stake_threshold: Default::default(),
                    validator_low_stake_grace_period: Default::default(),
                    extra_fields: Default::default(),
                },
                iota_system_admin_cap: Default::default(),
                reference_gas_price: Default::default(),
                validator_report_records: VecMap {
                    contents: Default::default(),
                },
                safe_mode: Default::default(),
                safe_mode_storage_charges: iota_types::balance::Balance::new(Default::default()),
                safe_mode_computation_rewards: iota_types::balance::Balance::new(Default::default()),
                safe_mode_storage_rebates: Default::default(),
                safe_mode_non_refundable_storage_fee: Default::default(),
                epoch_start_timestamp_ms: Default::default(),
                extra_fields: Default::default(),
            }
        }
    }
}
