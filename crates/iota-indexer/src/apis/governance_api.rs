// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeMap, str::FromStr, sync::Arc};

use async_trait::async_trait;
use cached::{Cached, SizedCache};
use diesel::r2d2::R2D2Connection;
use iota_json_rpc::{IotaRpcModule, governance_api::ValidatorExchangeRates};
use iota_json_rpc_api::GovernanceReadApiServer;
use iota_json_rpc_types::{
    DelegatedStake, DelegatedTimelockedStake, EpochInfo, IotaCommittee, IotaObjectDataFilter,
    StakeStatus, ValidatorApys,
};
use iota_open_rpc::Module;
use iota_types::{
    base_types::{IotaAddress, MoveObjectType, ObjectID},
    committee::EpochId,
    error::IotaError,
    governance::StakedIota,
    iota_serde::BigInt,
    iota_system_state::{PoolTokenExchangeRate, iota_system_state_summary::IotaSystemStateSummary},
    timelock::timelocked_staked_iota::TimelockedStakedIota,
};
use jsonrpsee::{RpcModule, core::RpcResult};
use serde::Deserialize;
use tokio::sync::Mutex;

use crate::{errors::IndexerError, indexer_reader::IndexerReader};

/// Maximum amount of staked objects for querying.
const MAX_QUERY_STAKED_OBJECTS: usize = 1000;

#[derive(Clone)]
pub struct GovernanceReadApi<T: R2D2Connection + 'static> {
    inner: IndexerReader<T>,
    exchange_rates_cache: Arc<Mutex<SizedCache<EpochId, Vec<ValidatorExchangeRates>>>>,
    validators_apys_cache: Arc<Mutex<SizedCache<EpochId, BTreeMap<IotaAddress, f64>>>>,
}

impl<T: R2D2Connection + 'static> GovernanceReadApi<T> {
    pub fn new(inner: IndexerReader<T>) -> Self {
        Self {
            inner,
            exchange_rates_cache: Arc::new(Mutex::new(SizedCache::with_size(1))),
            validators_apys_cache: Arc::new(Mutex::new(SizedCache::with_size(1))),
        }
    }

    /// Get a validator's APY by its address
    pub async fn get_validator_apy(
        &self,
        address: &IotaAddress,
    ) -> Result<Option<f64>, IndexerError> {
        let apys = self
            .validators_apys_map(self.get_validators_apy().await?)
            .await;
        Ok(apys.get(address).copied())
    }

    async fn get_validators_apy(&self) -> Result<ValidatorApys, IndexerError> {
        let system_state_summary: IotaSystemStateSummary =
            self.get_latest_iota_system_state().await?;
        let epoch = system_state_summary.epoch;

        let exchange_rate_table = self.exchange_rates(&system_state_summary).await?;

        let apys = iota_json_rpc::governance_api::calculate_apys(exchange_rate_table);

        Ok(ValidatorApys { apys, epoch })
    }

    pub async fn get_epoch_info(&self, epoch: Option<EpochId>) -> Result<EpochInfo, IndexerError> {
        match self
            .inner
            .spawn_blocking(move |this| this.get_epoch_info(epoch))
            .await
        {
            Ok(Some(epoch_info)) => Ok(epoch_info),
            Ok(None) => Err(IndexerError::InvalidArgument(format!(
                "Missing epoch {epoch:?}"
            ))),
            Err(e) => Err(e),
        }
    }

    async fn get_latest_iota_system_state(&self) -> Result<IotaSystemStateSummary, IndexerError> {
        self.inner
            .spawn_blocking(|this| this.get_latest_iota_system_state())
            .await
    }

    async fn get_stakes_by_ids(
        &self,
        ids: Vec<ObjectID>,
    ) -> Result<Vec<DelegatedStake>, IndexerError> {
        let mut stakes = vec![];
        for stored_object in self.inner.multi_get_objects_in_blocking_task(ids).await? {
            let object = iota_types::object::Object::try_from(stored_object)?;
            let stake_object = StakedIota::try_from(&object)?;
            stakes.push(stake_object);
        }

        self.get_delegated_stakes(stakes).await
    }

    async fn get_staked_by_owner(
        &self,
        owner: IotaAddress,
    ) -> Result<Vec<DelegatedStake>, IndexerError> {
        let mut stakes = vec![];
        for stored_object in self
            .inner
            .get_owned_objects_in_blocking_task(
                owner,
                Some(IotaObjectDataFilter::StructType(
                    MoveObjectType::staked_iota().into(),
                )),
                None,
                MAX_QUERY_STAKED_OBJECTS,
            )
            .await?
        {
            let object = iota_types::object::Object::try_from(stored_object)?;
            let stake_object = StakedIota::try_from(&object)?;
            stakes.push(stake_object);
        }

        self.get_delegated_stakes(stakes).await
    }

    async fn get_timelocked_staked_by_owner(
        &self,
        owner: IotaAddress,
    ) -> Result<Vec<DelegatedTimelockedStake>, IndexerError> {
        let mut stakes = vec![];
        for stored_object in self
            .inner
            .get_owned_objects_in_blocking_task(
                owner,
                Some(IotaObjectDataFilter::StructType(
                    MoveObjectType::timelocked_staked_iota().into(),
                )),
                None,
                MAX_QUERY_STAKED_OBJECTS,
            )
            .await?
        {
            let object = iota_types::object::Object::try_from(stored_object)?;
            let stake_object = TimelockedStakedIota::try_from(&object)?;
            stakes.push(stake_object);
        }

        self.get_delegated_timelocked_stakes(stakes).await
    }

    pub async fn get_delegated_stakes(
        &self,
        stakes: Vec<StakedIota>,
    ) -> Result<Vec<DelegatedStake>, IndexerError> {
        let pools = stakes
            .into_iter()
            .fold(BTreeMap::<_, Vec<_>>::new(), |mut pools, stake| {
                pools.entry(stake.pool_id()).or_default().push(stake);
                pools
            });

        let system_state_summary = self.get_latest_iota_system_state().await?;
        let epoch = system_state_summary.epoch;

        let rates = self
            .exchange_rates(&system_state_summary)
            .await?
            .into_iter()
            .chain(
                self.pending_and_candidate_validators_exchange_rate(&system_state_summary)
                    .await?
                    .into_iter(),
            )
            .map(|rates| (rates.pool_id, rates))
            .collect::<BTreeMap<_, _>>();

        let mut delegated_stakes = vec![];
        for (pool_id, stakes) in pools {
            // Rate table and rate can be null when the pool is not active
            let rate_table = rates.get(&pool_id).ok_or_else(|| {
                IndexerError::InvalidArgument(format!(
                    "Cannot find rates for staking pool {pool_id}"
                ))
            })?;
            let current_rate = rate_table.rates.first().map(|(_, rate)| rate);

            let mut delegations = vec![];
            for stake in stakes {
                let status = stake_status(
                    epoch,
                    stake.activation_epoch(),
                    stake.principal(),
                    rate_table,
                    current_rate,
                );

                delegations.push(iota_json_rpc_types::Stake {
                    staked_iota_id: stake.id(),
                    // TODO: this might change when we implement warm up period.
                    stake_request_epoch: stake.activation_epoch().saturating_sub(1),
                    stake_active_epoch: stake.activation_epoch(),
                    principal: stake.principal(),
                    status,
                })
            }
            delegated_stakes.push(DelegatedStake {
                validator_address: rate_table.address,
                staking_pool: pool_id,
                stakes: delegations,
            })
        }
        Ok(delegated_stakes)
    }

    pub async fn get_delegated_timelocked_stakes(
        &self,
        stakes: Vec<TimelockedStakedIota>,
    ) -> Result<Vec<DelegatedTimelockedStake>, IndexerError> {
        let pools = stakes
            .into_iter()
            .fold(BTreeMap::<_, Vec<_>>::new(), |mut pools, stake| {
                pools.entry(stake.pool_id()).or_default().push(stake);
                pools
            });

        let system_state_summary = self.get_latest_iota_system_state().await?;
        let epoch = system_state_summary.epoch;

        let rates = self
            .exchange_rates(&system_state_summary)
            .await?
            .into_iter()
            .map(|rates| (rates.pool_id, rates))
            .collect::<BTreeMap<_, _>>();

        let mut delegated_stakes = vec![];
        for (pool_id, stakes) in pools {
            // Rate table and rate can be null when the pool is not active
            let rate_table = rates.get(&pool_id).ok_or_else(|| {
                IndexerError::InvalidArgument(format!(
                    "Cannot find rates for staking pool {pool_id}"
                ))
            })?;
            let current_rate = rate_table.rates.first().map(|(_, rate)| rate);

            let mut delegations = vec![];
            for stake in stakes {
                let status = stake_status(
                    epoch,
                    stake.activation_epoch(),
                    stake.principal(),
                    rate_table,
                    current_rate,
                );

                delegations.push(iota_json_rpc_types::TimelockedStake {
                    timelocked_staked_iota_id: stake.id(),
                    // TODO: this might change when we implement warm up period.
                    stake_request_epoch: stake.activation_epoch().saturating_sub(1),
                    stake_active_epoch: stake.activation_epoch(),
                    principal: stake.principal(),
                    status,
                    expiration_timestamp_ms: stake.expiration_timestamp_ms(),
                    label: stake.label().clone(),
                })
            }
            delegated_stakes.push(DelegatedTimelockedStake {
                validator_address: rate_table.address,
                staking_pool: pool_id,
                stakes: delegations,
            })
        }
        Ok(delegated_stakes)
    }

    /// Cache a map representing the validators' APYs for this epoch
    async fn validators_apys_map(&self, apys: ValidatorApys) -> BTreeMap<IotaAddress, f64> {
        // check if the apys are already in the cache
        if let Some(cached_apys) = self
            .validators_apys_cache
            .lock()
            .await
            .cache_get(&apys.epoch)
        {
            return cached_apys.clone();
        }

        let ret = BTreeMap::from_iter(apys.apys.iter().map(|x| (x.address, x.apy)));
        // insert the apys into the cache
        self.validators_apys_cache
            .lock()
            .await
            .cache_set(apys.epoch, ret.clone());

        ret
    }

    // Get validator exchange rates
    async fn validator_exchange_rates(
        &self,
        tables: Vec<(IotaAddress, ObjectID, ObjectID, u64, bool)>,
    ) -> Result<Vec<ValidatorExchangeRates>, IndexerError> {
        if tables.is_empty() {
            return Ok(vec![]);
        };

        let mut exchange_rates = vec![];
        // Get exchange rates for each validator
        for (address, pool_id, exchange_rates_id, exchange_rates_size, active) in tables {
            let mut rates = vec![];
            for df in self
                .inner
                .get_dynamic_fields_raw_in_blocking_task(
                    exchange_rates_id,
                    None,
                    exchange_rates_size as usize,
                )
                .await?
            {
                let dynamic_field = df
                    .to_dynamic_field::<EpochId, PoolTokenExchangeRate>()
                    .ok_or_else(|| iota_types::error::IotaError::ObjectDeserialization {
                        error: "dynamic field malformed".to_owned(),
                    })?;

                rates.push((dynamic_field.name, dynamic_field.value));
            }

            rates.sort_by(|(a, _), (b, _)| a.cmp(b).reverse());

            exchange_rates.push(ValidatorExchangeRates {
                address,
                pool_id,
                active,
                rates,
            });
        }
        Ok(exchange_rates)
    }

    /// Cached exchange rates for validators for the given epoch, the cache size
    /// is 1, it will be cleared when the epoch changes. Rates are in
    /// descending order by epoch.
    pub async fn exchange_rates(
        &self,
        system_state_summary: &IotaSystemStateSummary,
    ) -> Result<Vec<ValidatorExchangeRates>, IndexerError> {
        let epoch = system_state_summary.epoch;

        let mut cache = self.exchange_rates_cache.lock().await;

        // Check if the exchange rates for the current epoch are cached
        if let Some(cached_rates) = cache.cache_get(&epoch) {
            return Ok(cached_rates.clone());
        }

        // Cache miss: compute exchange rates
        let exchange_rates = self.compute_exchange_rates(system_state_summary).await?;

        // Store in cache
        cache.cache_set(epoch, exchange_rates.clone());

        Ok(exchange_rates)
    }

    /// Compute Exchange Rates for Active & Inactive validators
    async fn compute_exchange_rates(
        &self,
        system_state_summary: &IotaSystemStateSummary,
    ) -> Result<Vec<ValidatorExchangeRates>, IndexerError> {
        // Get validator rate tables
        let mut tables = vec![];

        for validator in &system_state_summary.active_validators {
            tables.push((
                validator.iota_address,
                validator.staking_pool_id,
                validator.exchange_rates_id,
                validator.exchange_rates_size,
                true,
            ));
        }

        // Get inactive validator rate tables
        for df in self
            .inner
            .get_dynamic_fields_in_blocking_task(
                system_state_summary.inactive_pools_id,
                None,
                system_state_summary.inactive_pools_size as usize,
            )
            .await?
        {
            let pool_id: iota_types::id::ID = bcs::from_bytes(&df.bcs_name).map_err(|e| {
                iota_types::error::IotaError::ObjectDeserialization {
                    error: e.to_string(),
                }
            })?;
            let inactive_pools_id = system_state_summary.inactive_pools_id;
            let validator = self
                .inner
                .spawn_blocking(move |this| {
                    iota_types::iota_system_state::get_validator_from_table(
                        &this,
                        inactive_pools_id,
                        &pool_id,
                    )
                })
                .await?;
            tables.push((
                validator.iota_address,
                validator.staking_pool_id,
                validator.exchange_rates_id,
                validator.exchange_rates_size,
                false,
            ));
        }

        self.validator_exchange_rates(tables).await
    }

    /// Check if there is any `Pending` and `Candidate` validators and get its
    /// exchange rates, this two states of a validator lifecycle can manifest
    /// during an epoch or multiple ones, is essential to not cache this data,
    /// while exchange rates for `Active` and `Inactive` validators is OK to do
    /// so because those states are manifested on epoch change.
    pub async fn pending_and_candidate_validators_exchange_rate(
        &self,
        system_state_summary: &IotaSystemStateSummary,
    ) -> Result<Vec<ValidatorExchangeRates>, IndexerError> {
        // Try to find for any pending active validator
        let mut tables = self
            .inner
            .pending_active_validators()
            .await?
            .into_iter()
            .map(|pending_active_validator| {
                (
                    pending_active_validator.iota_address,
                    pending_active_validator.staking_pool_id,
                    pending_active_validator.exchange_rates_id,
                    pending_active_validator.exchange_rates_size,
                    false,
                )
            })
            .collect::<Vec<(IotaAddress, ObjectID, ObjectID, u64, bool)>>();

        // Get inactive validator rate tables
        for df in self
            .inner
            .get_dynamic_fields_in_blocking_task(
                system_state_summary.validator_candidates_id,
                None,
                system_state_summary.validator_candidates_size as usize,
            )
            .await?
        {
            let raw_address = String::deserialize(df.name.value)
                .map_err(|err| IotaError::from(err.to_string()))?;
            let validator_address = IotaAddress::from_str(&raw_address)?;

            let validator_candidates_id = system_state_summary.validator_candidates_id;

            let validator_candidate = self
                .inner
                .spawn_blocking(move |this| {
                    iota_types::iota_system_state::get_validator_from_table(
                        &this,
                        validator_candidates_id,
                        &validator_address,
                    )
                })
                .await?;

            tables.push((
                validator_candidate.iota_address,
                validator_candidate.staking_pool_id,
                validator_candidate.exchange_rates_id,
                validator_candidate.exchange_rates_size,
                false,
            ));
        }

        self.validator_exchange_rates(tables).await
    }
}

fn stake_status(
    epoch: u64,
    activation_epoch: u64,
    principal: u64,
    rate_table: &ValidatorExchangeRates,
    current_rate: Option<&PoolTokenExchangeRate>,
) -> StakeStatus {
    if epoch >= activation_epoch {
        let estimated_reward = if let Some(current_rate) = current_rate {
            let stake_rate = rate_table
                .rates
                .iter()
                .find_map(|(epoch, rate)| (*epoch == activation_epoch).then(|| rate.clone()))
                .unwrap_or_default();
            let estimated_reward =
                ((stake_rate.rate() / current_rate.rate()) - 1.0) * principal as f64;
            std::cmp::max(0, estimated_reward.round() as u64)
        } else {
            0
        };
        StakeStatus::Active { estimated_reward }
    } else {
        StakeStatus::Pending
    }
}

#[async_trait]
impl<T: R2D2Connection + 'static> GovernanceReadApiServer for GovernanceReadApi<T> {
    async fn get_stakes_by_ids(
        &self,
        staked_iota_ids: Vec<ObjectID>,
    ) -> RpcResult<Vec<DelegatedStake>> {
        self.get_stakes_by_ids(staked_iota_ids)
            .await
            .map_err(Into::into)
    }

    async fn get_stakes(&self, owner: IotaAddress) -> RpcResult<Vec<DelegatedStake>> {
        self.get_staked_by_owner(owner).await.map_err(Into::into)
    }

    async fn get_timelocked_stakes_by_ids(
        &self,
        timelocked_staked_iota_ids: Vec<ObjectID>,
    ) -> RpcResult<Vec<DelegatedTimelockedStake>> {
        let stakes = self
            .inner
            .multi_get_objects_in_blocking_task(timelocked_staked_iota_ids)
            .await?
            .into_iter()
            .map(|stored_object| {
                let object = iota_types::object::Object::try_from(stored_object)?;
                TimelockedStakedIota::try_from(&object).map_err(IndexerError::from)
            })
            .collect::<Result<Vec<_>, _>>()?;

        self.get_delegated_timelocked_stakes(stakes)
            .await
            .map_err(Into::into)
    }

    async fn get_timelocked_stakes(
        &self,
        owner: IotaAddress,
    ) -> RpcResult<Vec<DelegatedTimelockedStake>> {
        self.get_timelocked_staked_by_owner(owner)
            .await
            .map_err(Into::into)
    }

    async fn get_committee_info(&self, epoch: Option<BigInt<u64>>) -> RpcResult<IotaCommittee> {
        let epoch = self.get_epoch_info(epoch.as_deref().copied()).await?;
        Ok(epoch.committee().map_err(IndexerError::from)?.into())
    }

    async fn get_latest_iota_system_state(&self) -> RpcResult<IotaSystemStateSummary> {
        self.get_latest_iota_system_state()
            .await
            .map_err(Into::into)
    }

    async fn get_reference_gas_price(&self) -> RpcResult<BigInt<u64>> {
        let epoch = self.get_epoch_info(None).await?;
        Ok(BigInt::from(epoch.reference_gas_price.ok_or_else(
            || {
                IndexerError::PersistentStorageDataCorruption(
                    "missing latest reference gas price".to_owned(),
                )
            },
        )?))
    }

    async fn get_validators_apy(&self) -> RpcResult<ValidatorApys> {
        Ok(self.get_validators_apy().await?)
    }
}

impl<T: R2D2Connection> IotaRpcModule for GovernanceReadApi<T> {
    fn rpc(self) -> RpcModule<Self> {
        self.into_rpc()
    }

    fn rpc_doc_module() -> Module {
        iota_json_rpc_api::GovernanceReadApiOpenRpc::module_doc()
    }
}
