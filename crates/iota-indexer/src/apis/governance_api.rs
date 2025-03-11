// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeMap, fmt::Debug, sync::Arc};

use async_trait::async_trait;
use cached::{Cached, SizedCache};
use iota_json_rpc::{IotaRpcModule, governance_api::ValidatorExchangeRates};
use iota_json_rpc_api::GovernanceReadApiServer;
use iota_json_rpc_types::{
    DelegatedStake, DelegatedTimelockedStake, EpochInfo, IotaCommittee, IotaObjectDataFilter,
    StakeStatus, ValidatorApys,
};
use iota_open_rpc::Module;
use iota_types::{
    MoveTypeTagTrait,
    base_types::{IotaAddress, MoveObjectType, ObjectID},
    committee::EpochId,
    dynamic_field::DynamicFieldInfo,
    governance::StakedIota,
    id::ID,
    iota_serde::BigInt,
    iota_system_state::{
        PoolTokenExchangeRate,
        iota_system_state_summary::{IotaSystemStateSummary, IotaSystemStateSummaryV1},
    },
    timelock::timelocked_staked_iota::TimelockedStakedIota,
};
use jsonrpsee::{RpcModule, core::RpcResult};
use serde::{Serialize, de::DeserializeOwned};
use tokio::sync::Mutex;

use crate::{
    errors::IndexerError, indexer_reader::IndexerReader, types::IotaSystemStateSummaryView,
};

/// Maximum amount of staked objects for querying.
const MAX_QUERY_STAKED_OBJECTS: usize = 1000;

type ValidatorTable = (IotaAddress, ObjectID, ObjectID, u64, bool);

#[derive(Clone)]
pub struct GovernanceReadApi {
    inner: IndexerReader,
    exchange_rates_cache: Arc<Mutex<SizedCache<EpochId, Vec<ValidatorExchangeRates>>>>,
    validators_apys_cache: Arc<Mutex<SizedCache<EpochId, BTreeMap<IotaAddress, f64>>>>,
}

impl GovernanceReadApi {
    pub fn new(inner: IndexerReader) -> Self {
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
        let system_state_summary = self.get_latest_iota_system_state().await?;
        let epoch = system_state_summary.epoch();

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
        let epoch = system_state_summary.epoch();

        let (candidate_rates, pending_rates) = tokio::try_join!(
            self.candidate_validators_exchange_rate(&system_state_summary),
            self.pending_validators_exchange_rate()
        )?;

        let rates = self
            .exchange_rates(&system_state_summary)
            .await?
            .into_iter()
            .chain(candidate_rates.into_iter())
            .chain(pending_rates.into_iter())
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
        let epoch = system_state_summary.epoch();

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

    /// Get validator exchange rates
    async fn validator_exchange_rates(
        &self,
        tables: Vec<ValidatorTable>,
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

            // Rates for some epochs might be missing due to safe mode, we need to backfill
            // them.
            rates = backfill_rates(rates);

            exchange_rates.push(ValidatorExchangeRates {
                address,
                pool_id,
                active,
                rates,
            });
        }
        Ok(exchange_rates)
    }

    /// Caches exchange rates for validators for the given epoch, the cache size
    /// is 1, it will be cleared when the epoch changes. Rates are in
    /// descending order by epoch.
    pub async fn exchange_rates(
        &self,
        system_state_summary: &IotaSystemStateSummary,
    ) -> Result<Vec<ValidatorExchangeRates>, IndexerError> {
        let epoch = system_state_summary.epoch();

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
        let (active_rates, inactive_rates) = tokio::try_join!(
            self.active_validators_exchange_rate(system_state_summary),
            self.inactive_validators_exchange_rate(system_state_summary)
        )?;

        Ok(active_rates
            .into_iter()
            .chain(inactive_rates.into_iter())
            .collect())
    }

    /// Check for validators in the `Active` state and get its exchange rate
    async fn active_validators_exchange_rate(
        &self,
        system_state_summary: &IotaSystemStateSummary,
    ) -> Result<Vec<ValidatorExchangeRates>, IndexerError> {
        let tables = system_state_summary
            .active_validators()
            .iter()
            .map(|validator| {
                (
                    validator.iota_address,
                    validator.staking_pool_id,
                    validator.exchange_rates_id,
                    validator.exchange_rates_size,
                    true,
                )
            })
            .collect();

        self.validator_exchange_rates(tables).await
    }

    /// Check for validators in the `Inactive` state and get its exchange rate
    async fn inactive_validators_exchange_rate(
        &self,
        system_state_summary: &IotaSystemStateSummary,
    ) -> Result<Vec<ValidatorExchangeRates>, IndexerError> {
        let tables = self
            .validator_summary_from_system_state(
                system_state_summary.inactive_pools_id(),
                system_state_summary.inactive_pools_size(),
                |df| bcs::from_bytes::<ID>(&df.bcs_name).map_err(Into::into),
            )
            .await?;

        self.validator_exchange_rates(tables).await
    }

    /// Check for validators in the `Pending` state and get its exchange rate.
    /// For these validators, their exchange rates should not be cached as
    /// their state can occur during an epoch or across multiple ones. In
    /// contrast, exchange rates for `Active` and `Inactive` validators can
    /// be cached, as their state changes only at epoch change.
    pub async fn pending_validators_exchange_rate(
        &self,
    ) -> Result<Vec<ValidatorExchangeRates>, IndexerError> {
        // Try to find for any pending active validator
        let tables = self
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
            .collect::<Vec<ValidatorTable>>();

        self.validator_exchange_rates(tables).await
    }

    /// Check for validators in the `Candidate` state and get its exchange rate.
    /// For these validators, their exchange rates should not be cached as
    /// their state can occur during an epoch or across multiple ones. In
    /// contrast, exchange rates for `Active` and `Inactive` validators can
    /// be cached, as their state changes only at epoch change.
    pub async fn candidate_validators_exchange_rate(
        &self,
        system_state_summary: &IotaSystemStateSummary,
    ) -> Result<Vec<ValidatorExchangeRates>, IndexerError> {
        let tables = self
            .validator_summary_from_system_state(
                system_state_summary.validator_candidates_id(),
                system_state_summary.validator_candidates_size(),
                |df| bcs::from_bytes::<IotaAddress>(&df.bcs_name).map_err(Into::into),
            )
            .await?;

        self.validator_exchange_rates(tables).await
    }

    /// Fetches validator status information from `StateRead`.
    ///
    /// This makes sense for validators not included in
    /// `IotaSystemStateSummary`. `IotaSystemStateSummary` only contains
    /// information about `Active` validators. To retrieve information about
    /// `Inactive`, `Candidate`, and `Pending` validators, we need to access
    /// dynamic fields within specific Move tables.
    ///
    /// To retrieve validator status information, this function utilizes the
    /// corresponding `table_id` (an `ObjectID` value) and a `limit` to specify
    /// the number of records to fetch. Both the `table_id` and `limit` can
    /// be obtained from `IotaSystemStateSummary` in the caller.
    /// Additionally, keys are extracted from the table `DynamicFieldInfo`
    /// values according to the `key` closure. This helps in identifying the
    /// specific validator within the table.
    ///
    /// # Example
    ///
    /// ```text
    /// // Get inactive validators
    /// let system_state_summary = self.get_latest_iota_system_state().await?;
    /// let _ = self.validator_summary_from_system_state(
    ///        // ID of the object that maps from a staking pool ID to the inactive validator that has that pool as its staking pool
    ///        system_state_summary.inactive_pools_id(),
    ///        // Number of inactive staking pools
    ///        system_state_summary.inactive_pools_size(),
    ///        // Extract the `ID` of the `Inactive` validator from the `DynamicFieldInfo` in the `system_state_summary.inactive_pools_id` table
    ///        |df| bcs::from_bytes::<ID>(&df.bcs_name).map_err(Into::into),
    /// ).await?;
    /// ```
    ///
    /// # Example
    ///
    /// ```text
    /// // Get candidate validators
    /// let system_state_summary = self.get_latest_iota_system_state().await?;
    /// let _ = self.validator_summary_from_system_state(
    ///        // ID of the object that stores preactive validators, mapping their addresses to their Validator structs
    ///        system_state_summary.validator_candidates_id(),
    ///        // Number of preactive validators
    ///        system_state_summary.validator_candidates_size(),
    ///        // Extract the `IotaAddress` of the `Candidate` validator from the `DynamicFieldInfo` in the `system_state_summary.validator_candidates_id` table
    ///        |df| bcs::from_bytes::<IotaAddress>(&df.bcs_name).map_err(Into::into),
    /// ).await?;
    /// ```
    async fn validator_summary_from_system_state<K, F>(
        &self,
        table_id: ObjectID,
        validator_size: u64,
        key: F,
    ) -> Result<Vec<ValidatorTable>, IndexerError>
    where
        F: Fn(DynamicFieldInfo) -> Result<K, IndexerError>,
        K: MoveTypeTagTrait + Serialize + DeserializeOwned + Debug + Send + 'static,
    {
        let dynamic_fields = self
            .inner
            .get_dynamic_fields_in_blocking_task(table_id, None, validator_size as usize)
            .await?;

        let mut tables = Vec::with_capacity(dynamic_fields.len());

        for df in dynamic_fields {
            let key = key(df)?;
            let validator_candidate = self
                .inner
                .spawn_blocking(move |this| {
                    iota_types::iota_system_state::get_validator_from_table(&this, table_id, &key)
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

        Ok(tables)
    }
}

/// Backfill missing rates for some epochs due to safe mode. If a rate is
/// missing for epoch e, we will use the rate for epoch e-1 to fill it. Rates
/// returned are in descending order by epoch.
fn backfill_rates(
    mut rates: Vec<(EpochId, PoolTokenExchangeRate)>,
) -> Vec<(EpochId, PoolTokenExchangeRate)> {
    if rates.is_empty() {
        return rates;
    }
    // ensure epochs are processed in increasing order
    rates.sort_unstable_by_key(|(epoch_id, _)| *epoch_id);

    // Check if there are any gaps in the epochs
    let (min_epoch, _) = rates.first().expect("rates should not be empty");
    let (max_epoch, _) = rates.last().expect("rates should not be empty");
    let expected_len = (max_epoch - min_epoch + 1) as usize;
    let current_len = rates.len();

    // Only perform backfilling if there are gaps
    if current_len == expected_len {
        rates.reverse();
        return rates;
    }

    let mut filled_rates: Vec<(EpochId, PoolTokenExchangeRate)> = Vec::with_capacity(expected_len);
    let mut missing_rates = Vec::with_capacity(expected_len - current_len);
    for (epoch_id, rate) in rates {
        // fill gaps between the last processed epoch and the current one
        if let Some((prev_epoch_id, prev_rate)) = filled_rates.last() {
            for missing_epoch_id in prev_epoch_id + 1..epoch_id {
                missing_rates.push((missing_epoch_id, prev_rate.clone()));
            }
        };

        // append any missing_rates before adding the current epoch.
        // if empty, nothing gets appended.
        // if not empty, it will be empty afterwards because it was moved into
        // filled_rates
        filled_rates.append(&mut missing_rates);
        filled_rates.push((epoch_id, rate));
    }
    filled_rates.reverse();
    filled_rates
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
impl GovernanceReadApiServer for GovernanceReadApi {
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

    async fn get_latest_iota_system_state_v2(&self) -> RpcResult<IotaSystemStateSummary> {
        Ok(self.get_latest_iota_system_state().await?)
    }

    async fn get_latest_iota_system_state(&self) -> RpcResult<IotaSystemStateSummaryV1> {
        Ok(self
            .get_latest_iota_system_state()
            .await?
            .try_into()
            .map_err(IndexerError::from)?)
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

impl IotaRpcModule for GovernanceReadApi {
    fn rpc(self) -> RpcModule<Self> {
        self.into_rpc()
    }

    fn rpc_doc_module() -> Module {
        iota_json_rpc_api::GovernanceReadApiOpenRpc::module_doc()
    }
}

#[cfg(test)]
mod tests {
    use iota_types::iota_system_state::PoolTokenExchangeRate;

    use super::*;
    #[test]
    fn test_backfill_rates_empty() {
        let rates = vec![];
        assert_eq!(backfill_rates(rates), vec![]);
    }

    #[test]
    fn test_backfill_single_rate() {
        let rate1 = PoolTokenExchangeRate::new_for_testing(100, 100);
        let rates = vec![(1, rate1.clone())];
        let expected = vec![(1, rate1)];
        assert_eq!(backfill_rates(rates), expected);
    }

    #[test]
    fn test_backfill_rates_no_gaps() {
        let rate1 = PoolTokenExchangeRate::new_for_testing(100, 100);
        let rate2 = PoolTokenExchangeRate::new_for_testing(200, 220);
        let rate3 = PoolTokenExchangeRate::new_for_testing(300, 330);
        let rates = vec![(2, rate2.clone()), (3, rate3.clone()), (1, rate1.clone())];
        let expected: Vec<(u64, PoolTokenExchangeRate)> =
            vec![(3, rate3.clone()), (2, rate2), (1, rate1)];
        assert_eq!(backfill_rates(rates), expected);
    }

    #[test]
    fn test_backfill_rates_with_gaps() {
        let rate1 = PoolTokenExchangeRate::new_for_testing(100, 100);
        let rate3 = PoolTokenExchangeRate::new_for_testing(300, 330);
        let rate5 = PoolTokenExchangeRate::new_for_testing(500, 550);
        let rates = vec![(3, rate3.clone()), (1, rate1.clone()), (5, rate5.clone())];
        let expected = vec![
            (5, rate5.clone()),
            (4, rate3.clone()),
            (3, rate3.clone()),
            (2, rate1.clone()),
            (1, rate1),
        ];
        assert_eq!(backfill_rates(rates), expected);
    }

    #[test]
    fn test_backfill_rates_missing_middle_epoch() {
        let rate1 = PoolTokenExchangeRate::new_for_testing(100, 100);
        let rate3 = PoolTokenExchangeRate::new_for_testing(300, 330);
        let rates = vec![(1, rate1.clone()), (3, rate3.clone())];
        let expected = vec![(3, rate3), (2, rate1.clone()), (1, rate1)];
        assert_eq!(backfill_rates(rates), expected);
    }

    #[test]
    fn test_backfill_rates_missing_middle_epochs() {
        let rate1 = PoolTokenExchangeRate::new_for_testing(100, 100);
        let rate4 = PoolTokenExchangeRate::new_for_testing(400, 440);
        let rates = vec![(1, rate1.clone()), (4, rate4.clone())];
        let expected = vec![
            (4, rate4),
            (3, rate1.clone()),
            (2, rate1.clone()),
            (1, rate1),
        ];
        assert_eq!(backfill_rates(rates), expected);
    }

    #[test]
    fn test_backfill_rates_unordered_input() {
        let rate1 = PoolTokenExchangeRate::new_for_testing(100, 100);
        let rate3 = PoolTokenExchangeRate::new_for_testing(300, 330);
        let rate4 = PoolTokenExchangeRate::new_for_testing(400, 440);
        let rates = vec![(3, rate3.clone()), (1, rate1.clone()), (4, rate4.clone())];
        let expected = vec![(4, rate4), (3, rate3), (2, rate1.clone()), (1, rate1)];
        assert_eq!(backfill_rates(rates), expected);
    }
}
