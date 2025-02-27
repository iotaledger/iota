// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{cmp::max, collections::BTreeMap, fmt::Debug, sync::Arc};

use async_trait::async_trait;
use cached::{SizedCache, proc_macro::cached};
use iota_core::authority::AuthorityState;
use iota_json_rpc_api::{
    GovernanceReadApiOpenRpc, GovernanceReadApiServer, JsonRpcMetrics, error_object_from_rpc,
};
use iota_json_rpc_types::{
    DelegatedStake, DelegatedTimelockedStake, IotaCommittee, Stake, StakeStatus, TimelockedStake,
    ValidatorApy, ValidatorApys,
};
use iota_metrics::spawn_monitored_task;
use iota_open_rpc::Module;
use iota_types::{
    MoveTypeTagTrait,
    base_types::{IotaAddress, ObjectID},
    committee::EpochId,
    dynamic_field::{DynamicFieldInfo, get_dynamic_field_from_store},
    error::{IotaError, UserInputError},
    governance::StakedIota,
    id::ID,
    iota_serde::BigInt,
    iota_system_state::{
        IotaSystemState, IotaSystemStateTrait, PoolTokenExchangeRate, get_validator_from_table,
        iota_system_state_summary::IotaSystemStateSummary,
    },
    object::{Object, ObjectRead},
    timelock::timelocked_staked_iota::TimelockedStakedIota,
};
use itertools::Itertools;
use jsonrpsee::{RpcModule, core::RpcResult};
use serde::{Serialize, de::DeserializeOwned};
use statrs::statistics::{Data, Median};
use tracing::{info, instrument};

use crate::{
    IotaRpcModule, ObjectProvider,
    authority_state::StateRead,
    error::{Error, IotaRpcInputError, RpcInterimResult},
    logger::FutureWithTracing as _,
};

type ValidatorTable = (IotaAddress, ObjectID, ObjectID, u64, bool);

#[derive(Clone)]
pub struct GovernanceReadApi {
    state: Arc<dyn StateRead>,
    pub metrics: Arc<JsonRpcMetrics>,
}

impl GovernanceReadApi {
    pub fn new(state: Arc<AuthorityState>, metrics: Arc<JsonRpcMetrics>) -> Self {
        Self { state, metrics }
    }

    async fn get_staked_iota(&self, owner: IotaAddress) -> Result<Vec<StakedIota>, Error> {
        let state = self.state.clone();
        let result =
            spawn_monitored_task!(async move { state.get_staked_iota(owner).await }).await??;

        self.metrics
            .get_stake_iota_result_size
            .report(result.len() as u64);
        self.metrics
            .get_stake_iota_result_size_total
            .inc_by(result.len() as u64);
        Ok(result)
    }

    async fn get_timelocked_staked_iota(
        &self,
        owner: IotaAddress,
    ) -> Result<Vec<TimelockedStakedIota>, Error> {
        let state = self.state.clone();
        let result =
            spawn_monitored_task!(async move { state.get_timelocked_staked_iota(owner).await })
                .await??;

        self.metrics
            .get_stake_iota_result_size
            .report(result.len() as u64);
        self.metrics
            .get_stake_iota_result_size_total
            .inc_by(result.len() as u64);
        Ok(result)
    }

    async fn get_stakes_by_ids(
        &self,
        staked_iota_ids: Vec<ObjectID>,
    ) -> Result<Vec<DelegatedStake>, Error> {
        let state = self.state.clone();
        let stakes_read = spawn_monitored_task!(async move {
            staked_iota_ids
                .iter()
                .map(|id| state.get_object_read(id))
                .collect::<Result<Vec<_>, _>>()
        })
        .await??;

        if stakes_read.is_empty() {
            return Ok(vec![]);
        }

        let stakes: Vec<(StakedIota, bool)> = self
            .stakes_with_status(stakes_read.into_iter())
            .await?
            .into_iter()
            .map(|(o, b)| StakedIota::try_from(&o).map(|stake| (stake, b)))
            .collect::<Result<_, _>>()?;

        self.get_delegated_stakes(stakes).await
    }

    async fn get_stakes(&self, owner: IotaAddress) -> Result<Vec<DelegatedStake>, Error> {
        let timer = self.metrics.get_stake_iota_latency.start_timer();
        let stakes = self.get_staked_iota(owner).await?;
        if stakes.is_empty() {
            return Ok(vec![]);
        }
        drop(timer);

        let _timer = self.metrics.get_delegated_iota_latency.start_timer();

        let self_clone = self.clone();
        spawn_monitored_task!(
            self_clone.get_delegated_stakes(stakes.into_iter().map(|s| (s, true)).collect())
        )
        .await?
    }

    async fn get_timelocked_stakes_by_ids(
        &self,
        timelocked_staked_iota_ids: Vec<ObjectID>,
    ) -> Result<Vec<DelegatedTimelockedStake>, Error> {
        let state = self.state.clone();
        let stakes_read = spawn_monitored_task!(async move {
            timelocked_staked_iota_ids
                .iter()
                .map(|id| state.get_object_read(id))
                .collect::<Result<Vec<_>, _>>()
        })
        .await??;

        if stakes_read.is_empty() {
            return Ok(vec![]);
        }

        let stakes: Vec<(TimelockedStakedIota, bool)> = self
            .stakes_with_status(stakes_read.into_iter())
            .await?
            .into_iter()
            .map(|(o, b)| TimelockedStakedIota::try_from(&o).map(|stake| (stake, b)))
            .collect::<Result<_, _>>()?;

        self.get_delegated_timelocked_stakes(stakes).await
    }

    async fn get_timelocked_stakes(
        &self,
        owner: IotaAddress,
    ) -> Result<Vec<DelegatedTimelockedStake>, Error> {
        let timer = self.metrics.get_stake_iota_latency.start_timer();
        let stakes = self.get_timelocked_staked_iota(owner).await?;
        if stakes.is_empty() {
            return Ok(vec![]);
        }
        drop(timer);

        let _timer = self.metrics.get_delegated_iota_latency.start_timer();

        let self_clone = self.clone();
        spawn_monitored_task!(
            self_clone
                .get_delegated_timelocked_stakes(stakes.into_iter().map(|s| (s, true)).collect())
        )
        .await?
    }

    async fn get_delegated_stakes(
        &self,
        stakes: Vec<(StakedIota, bool)>,
    ) -> Result<Vec<DelegatedStake>, Error> {
        let pools = stakes.into_iter().fold(
            BTreeMap::<_, Vec<_>>::new(),
            |mut pools, (stake, exists)| {
                pools
                    .entry(stake.pool_id())
                    .or_default()
                    .push((stake, exists));
                pools
            },
        );

        let system_state = self.get_system_state()?;
        let system_state_summary = system_state.clone().into_iota_system_state_summary();

        let rates = exchange_rates(&self.state, system_state_summary.epoch)
            .await?
            .into_iter()
            // Try to find for any candidate validator exchange rate
            .chain(candidate_validators_exchange_rate(&self.state)?.into_iter())
            // Try to find for any pending validator exchange rate
            .chain(pending_validators_exchange_rate(&self.state)?.into_iter())
            .map(|rates| (rates.pool_id, rates))
            .collect::<BTreeMap<_, _>>();

        let mut delegated_stakes = vec![];
        for (pool_id, stakes) in pools {
            // Rate table and rate can be null when the pool is not active
            let rate_table = rates.get(&pool_id).ok_or_else(|| {
                IotaRpcInputError::GenericNotFound(format!(
                    "Cannot find rates for staking pool {pool_id}"
                ))
            })?;
            let current_rate = rate_table.rates.first().map(|(_, rate)| rate);

            let mut delegations = vec![];
            for (stake, exists) in stakes {
                let status = stake_status(
                    system_state_summary.epoch,
                    stake.activation_epoch(),
                    stake.principal(),
                    exists,
                    current_rate,
                    rate_table,
                );
                delegations.push(Stake {
                    staked_iota_id: stake.id(),
                    // TODO: this might change when we implement warm up period.
                    stake_request_epoch: stake.activation_epoch() - 1,
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

    async fn get_delegated_timelocked_stakes(
        &self,
        stakes: Vec<(TimelockedStakedIota, bool)>,
    ) -> Result<Vec<DelegatedTimelockedStake>, Error> {
        let pools = stakes.into_iter().fold(
            BTreeMap::<_, Vec<_>>::new(),
            |mut pools, (stake, exists)| {
                pools
                    .entry(stake.pool_id())
                    .or_default()
                    .push((stake, exists));
                pools
            },
        );

        let system_state = self.get_system_state()?;
        let system_state_summary: IotaSystemStateSummary =
            system_state.clone().into_iota_system_state_summary();

        let rates = exchange_rates(&self.state, system_state_summary.epoch)
            .await?
            .into_iter()
            .map(|rates| (rates.pool_id, rates))
            .collect::<BTreeMap<_, _>>();

        let mut delegated_stakes = vec![];
        for (pool_id, stakes) in pools {
            // Rate table and rate can be null when the pool is not active
            let rate_table = rates.get(&pool_id).ok_or_else(|| {
                IotaRpcInputError::GenericNotFound(format!(
                    "Cannot find rates for staking pool {pool_id}"
                ))
            })?;
            let current_rate = rate_table.rates.first().map(|(_, rate)| rate);

            let mut delegations = vec![];
            for (stake, exists) in stakes {
                let status = stake_status(
                    system_state_summary.epoch,
                    stake.activation_epoch(),
                    stake.principal(),
                    exists,
                    current_rate,
                    rate_table,
                );
                delegations.push(TimelockedStake {
                    timelocked_staked_iota_id: stake.id(),
                    // TODO: this might change when we implement warm up period.
                    stake_request_epoch: stake.activation_epoch() - 1,
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

    async fn stakes_with_status(
        &self,
        iter: impl Iterator<Item = ObjectRead>,
    ) -> Result<Vec<(Object, bool)>, Error> {
        let mut stakes = vec![];

        for stake in iter {
            match stake {
                ObjectRead::Exists(_, o, _) => stakes.push((o, true)),
                ObjectRead::Deleted((object_id, version, _)) => {
                    let Some(o) = self
                        .state
                        .find_object_lt_or_eq_version(&object_id, &version.one_before().unwrap())
                        .await?
                    else {
                        Err(IotaRpcInputError::UserInput(
                            UserInputError::ObjectNotFound {
                                object_id,
                                version: None,
                            },
                        ))?
                    };
                    stakes.push((o, false));
                }
                ObjectRead::NotExists(id) => Err(IotaRpcInputError::UserInput(
                    UserInputError::ObjectNotFound {
                        object_id: id,
                        version: None,
                    },
                ))?,
            }
        }

        Ok(stakes)
    }

    fn get_system_state(&self) -> Result<IotaSystemState, Error> {
        Ok(self.state.get_system_state()?)
    }
}

#[async_trait]
impl GovernanceReadApiServer for GovernanceReadApi {
    #[instrument(skip(self))]
    async fn get_stakes_by_ids(
        &self,
        staked_iota_ids: Vec<ObjectID>,
    ) -> RpcResult<Vec<DelegatedStake>> {
        self.get_stakes_by_ids(staked_iota_ids).trace().await
    }

    #[instrument(skip(self))]
    async fn get_stakes(&self, owner: IotaAddress) -> RpcResult<Vec<DelegatedStake>> {
        self.get_stakes(owner).trace().await
    }

    #[instrument(skip(self))]
    async fn get_timelocked_stakes_by_ids(
        &self,
        timelocked_staked_iota_ids: Vec<ObjectID>,
    ) -> RpcResult<Vec<DelegatedTimelockedStake>> {
        self.get_timelocked_stakes_by_ids(timelocked_staked_iota_ids)
            .trace()
            .await
    }

    #[instrument(skip(self))]
    async fn get_timelocked_stakes(
        &self,
        owner: IotaAddress,
    ) -> RpcResult<Vec<DelegatedTimelockedStake>> {
        self.get_timelocked_stakes(owner).trace().await
    }

    #[instrument(skip(self))]
    async fn get_committee_info(&self, epoch: Option<BigInt<u64>>) -> RpcResult<IotaCommittee> {
        async move {
            self.state
                .get_or_latest_committee(epoch)
                .map(|committee| committee.into())
                .map_err(Error::from)
        }
        .trace()
        .await
    }

    #[instrument(skip(self))]
    async fn get_latest_iota_system_state(&self) -> RpcResult<IotaSystemStateSummary> {
        async move {
            Ok(self
                .state
                .get_system_state()
                .map_err(Error::from)?
                .into_iota_system_state_summary())
        }
        .trace()
        .await
    }

    #[instrument(skip(self))]
    async fn get_reference_gas_price(&self) -> RpcResult<BigInt<u64>> {
        async move {
            let epoch_store = self.state.load_epoch_store_one_call_per_task();
            Ok(epoch_store.reference_gas_price().into())
        }
        .trace()
        .await
    }

    #[instrument(skip(self))]
    async fn get_validators_apy(&self) -> RpcResult<ValidatorApys> {
        info!("get_validator_apy");
        let system_state_summary: IotaSystemStateSummary =
            self.get_latest_iota_system_state().await?;

        let exchange_rate_table = exchange_rates(&self.state, system_state_summary.epoch)
            .await
            .map_err(|e| error_object_from_rpc(e.into()))?;

        let apys = calculate_apys(exchange_rate_table);

        Ok(ValidatorApys {
            apys,
            epoch: system_state_summary.epoch,
        })
    }
}

pub fn calculate_apys(exchange_rate_table: Vec<ValidatorExchangeRates>) -> Vec<ValidatorApy> {
    let mut apys = vec![];

    for rates in exchange_rate_table.into_iter().filter(|r| r.active) {
        let exchange_rates = rates.rates.iter().map(|(_, rate)| rate);

        let median_apy = median_apy_from_exchange_rates(exchange_rates);
        apys.push(ValidatorApy {
            address: rates.address,
            apy: median_apy,
        });
    }
    apys
}

/// Calculate the APY for a validator based on the exchange rates of the staking
/// pool.
///
/// The calculation uses the median value of the sample, to filter out
/// outliers introduced by large staking/unstaking events.
pub fn median_apy_from_exchange_rates<'er>(
    exchange_rates: impl DoubleEndedIterator<Item = &'er PoolTokenExchangeRate> + Clone,
) -> f64 {
    // rates are sorted by epoch in descending order.
    let rates = exchange_rates.clone().dropping(1);
    let rates_next = exchange_rates.dropping_back(1);
    let apys = rates
        .zip(rates_next)
        .filter_map(|(er, er_next)| {
            let apy = calculate_apy(er, er_next);
            (apy > 0.0).then_some(apy)
        })
        .take(90)
        .collect::<Vec<_>>();

    if apys.is_empty() {
        // not enough data points
        0.0
    } else {
        Data::new(apys).median()
    }
}

/// Calculate the APY by the exchange rate of two consecutive epochs
/// (`er`, `er_next`).
///
/// The formula used is `APY_e = (er.rate - er_next.rate) / er.rate * 365`
fn calculate_apy(er: &PoolTokenExchangeRate, er_next: &PoolTokenExchangeRate) -> f64 {
    ((er.rate() - er_next.rate()) / er_next.rate()) * 365.0
}

fn stake_status(
    epoch: u64,
    activation_epoch: u64,
    principal: u64,
    exists: bool,
    current_rate: Option<&PoolTokenExchangeRate>,
    rate_table: &ValidatorExchangeRates,
) -> StakeStatus {
    if !exists {
        StakeStatus::Unstaked
    } else if epoch >= activation_epoch {
        // TODO: use dev_inspect to call a move function to get the estimated reward
        let estimated_reward = if let Some(current_rate) = current_rate {
            let stake_rate = rate_table
                .rates
                .iter()
                .find_map(|(epoch, rate)| (*epoch == activation_epoch).then(|| rate.clone()))
                .unwrap_or_default();
            let estimated_reward =
                ((stake_rate.rate() / current_rate.rate()) - 1.0) * principal as f64;
            max(0, estimated_reward.round() as u64)
        } else {
            0
        };
        StakeStatus::Active { estimated_reward }
    } else {
        StakeStatus::Pending
    }
}

/// Cached exchange rates for validators for the given epoch, the cache size is
/// 1, it will be cleared when the epoch changes. rates are in descending order
/// by epoch.
#[cached(
    type = "SizedCache<EpochId, Vec<ValidatorExchangeRates>>",
    create = "{ SizedCache::with_size(1) }",
    convert = "{ _current_epoch }",
    result = true
)]
async fn exchange_rates(
    state: &Arc<dyn StateRead>,
    _current_epoch: EpochId,
) -> RpcInterimResult<Vec<ValidatorExchangeRates>> {
    Ok(active_validators_exchange_rates(state)?
        .into_iter()
        .chain(inactive_validators_exchange_rates(state)?.into_iter())
        .collect())
}

/// Get validator exchange rates
fn validator_exchange_rates(
    state: &Arc<dyn StateRead>,
    tables: Vec<ValidatorTable>,
) -> RpcInterimResult<Vec<ValidatorExchangeRates>> {
    if tables.is_empty() {
        return Ok(vec![]);
    };

    let mut exchange_rates = vec![];
    // Get exchange rates for each validator
    for (address, pool_id, exchange_rates_id, exchange_rates_size, active) in tables {
        let mut rates = state
            .get_dynamic_fields(exchange_rates_id, None, exchange_rates_size as usize)?
            .into_iter()
            .map(|(_object_id, df)| {
                let epoch: EpochId = bcs::from_bytes(&df.bcs_name).map_err(|e| {
                    IotaError::ObjectDeserialization {
                        error: e.to_string(),
                    }
                })?;

                let exchange_rate: PoolTokenExchangeRate = get_dynamic_field_from_store(
                    &state.get_object_store().as_ref(),
                    exchange_rates_id,
                    &epoch,
                )?;

                Ok::<_, IotaError>((epoch, exchange_rate))
            })
            .collect::<Result<Vec<_>, _>>()?;

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

/// Check for validators in the `Active` state and get its exchange rate
fn active_validators_exchange_rates(
    state: &Arc<dyn StateRead>,
) -> RpcInterimResult<Vec<ValidatorExchangeRates>> {
    let system_state_summary = state.get_system_state()?.into_iota_system_state_summary();

    let tables = system_state_summary
        .active_validators
        .into_iter()
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

    validator_exchange_rates(state, tables)
}

/// Check for validators in the `Inactive` state and get its exchange rate
fn inactive_validators_exchange_rates(
    state: &Arc<dyn StateRead>,
) -> RpcInterimResult<Vec<ValidatorExchangeRates>> {
    let system_state_summary = state.get_system_state()?.into_iota_system_state_summary();

    let tables = validator_summary_from_system_state(
        state,
        system_state_summary.inactive_pools_id,
        system_state_summary.inactive_pools_size,
        |df| bcs::from_bytes::<ID>(&df.bcs_name).map_err(Into::into),
    )?;

    validator_exchange_rates(state, tables)
}

/// Check for validators in the `Pending` state and get its exchange rate. For
/// these validators, their exchange rates should not be cached as their state
/// can occur during an epoch or across multiple ones. In contrast, exchange
/// rates for `Active` and `Inactive` validators can be cached, as their state
/// changes only at epoch change.
fn pending_validators_exchange_rate(
    state: &Arc<dyn StateRead>,
) -> RpcInterimResult<Vec<ValidatorExchangeRates>> {
    let system_state = state.get_system_state()?;
    let object_store = state.get_object_store();

    // Try to find for any pending active validator
    let tables = system_state
        .get_pending_active_validators(object_store)?
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

    validator_exchange_rates(state, tables)
}

/// Check for validators in the `Candidate` state and get its exchange rate. For
/// these validators, their exchange rates should not be cached as their state
/// can occur during an epoch or across multiple ones. In contrast, exchange
/// rates for `Active` and `Inactive` validators can be cached, as their state
/// changes only at epoch change.
fn candidate_validators_exchange_rate(
    state: &Arc<dyn StateRead>,
) -> RpcInterimResult<Vec<ValidatorExchangeRates>> {
    let system_state_summary = state.get_system_state()?.into_iota_system_state_summary();

    // From validator_candidates_id table get validator info using as key its
    // IotaAddress
    let tables = validator_summary_from_system_state(
        state,
        system_state_summary.validator_candidates_id,
        system_state_summary.validator_candidates_size,
        |df| bcs::from_bytes::<IotaAddress>(&df.bcs_name).map_err(Into::into),
    )?;

    validator_exchange_rates(state, tables)
}

/// Fetches validator status information from `StateRead`.
///
/// This makes sense for validators not included in `IotaSystemStateSummary`.
/// `IotaSystemStateSummary` only contains information about `Active`
/// validators. To retrieve information about `Inactive`, `Candidate`, and
/// `Pending` validators, we need to access dynamic fields within specific
/// Move tables.
///
/// To retrieve validator status information, this function utilizes the
/// corresponding `table_id` (an `ObjectID` value) and a `limit` to specify the
/// number of records to fetch. Both the `table_id` and `limit` can be obtained
/// from `IotaSystemStateSummary` in the caller. Additionally, keys are
/// extracted from the table `DynamicFieldInfo` values according to the `key`
/// closure. This helps in identifying the specific validator within the table.
///
/// # Example
///
/// ```text
/// // Get inactive validators
/// let system_state_summary = state.get_system_state()?.into_iota_system_state_summary();
/// let _ = validator_summary_from_system_state(
///     state,
///     // ID of the object that maps from a staking pool ID to the inactive validator that has that pool as its staking pool.
///     system_state_summary.inactive_pools_id,
///     // Number of inactive staking pools.
///     system_state_summary.inactive_pools_size,
///     // Extract the `ID` of the `Inactive` validator from the `DynamicFieldInfo` in the `system_state_summary.inactive_pools_id` table
///     |df| bcs::from_bytes::<ID>(&df.bcs_name).map_err(Into::into),
/// ).unwrap();
/// ```
///
/// # Example
///
/// ```text
/// // Get candidate validators
/// let system_state_summary = state.get_system_state()?.into_iota_system_state_summary();
/// let _ = validator_summary_from_system_state(
///     state,
///     // ID of the object that stores preactive validators, mapping their addresses to their Validator structs
///     system_state_summary.validator_candidates_id,
///     // Number of preactive validators
///     system_state_summary.validator_candidates_size,
///     // Extract the `IotaAddress` of the `Candidate` validator from the `DynamicFieldInfo` in the `system_state_summary.validator_candidates_id` table
///     |df| bcs::from_bytes::<IotaAddress>(&df.bcs_name).map_err(Into::into),
/// ).unwrap();
/// ```
fn validator_summary_from_system_state<K, F>(
    state: &Arc<dyn StateRead>,
    table_id: ObjectID,
    limit: u64,
    key: F,
) -> RpcInterimResult<Vec<ValidatorTable>>
where
    F: Fn(DynamicFieldInfo) -> RpcInterimResult<K>,
    K: MoveTypeTagTrait + Serialize + DeserializeOwned + Debug,
{
    let object_store = state.get_object_store();

    state
        .get_dynamic_fields(table_id, None, limit as usize)?
        .into_iter()
        .map(|(_object_id, df)| {
            let validator_summary = get_validator_from_table(object_store, table_id, &key(df)?)?;

            Ok((
                validator_summary.iota_address,
                validator_summary.staking_pool_id,
                validator_summary.exchange_rates_id,
                validator_summary.exchange_rates_size,
                false,
            ))
        })
        .collect()
}

#[derive(Clone, Debug)]
pub struct ValidatorExchangeRates {
    pub address: IotaAddress,
    pub pool_id: ObjectID,
    pub active: bool,
    pub rates: Vec<(EpochId, PoolTokenExchangeRate)>,
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

impl IotaRpcModule for GovernanceReadApi {
    fn rpc(self) -> RpcModule<Self> {
        self.into_rpc()
    }

    fn rpc_doc_module() -> Module {
        GovernanceReadApiOpenRpc::module_doc()
    }
}

#[cfg(test)]
mod tests {
    use iota_types::iota_system_state::PoolTokenExchangeRate;

    use super::*;

    #[test]
    fn calculate_apys_with_outliers() {
        let file =
            std::fs::File::open("src/unit_tests/data/validator_exchange_rate/rates.json").unwrap();
        let rates: BTreeMap<String, Vec<(u64, PoolTokenExchangeRate)>> =
            serde_json::from_reader(file).unwrap();

        let mut address_map = BTreeMap::new();

        let exchange_rates = rates
            .into_iter()
            .map(|(validator, rates)| {
                let address = IotaAddress::random_for_testing_only();
                address_map.insert(address, validator);
                ValidatorExchangeRates {
                    address,
                    pool_id: ObjectID::random(),
                    active: true,
                    rates,
                }
            })
            .collect();

        let apys = calculate_apys(exchange_rates);

        for apy in &apys {
            println!("{}: {}", address_map[&apy.address], apy.apy);
            assert!(apy.apy < 0.25)
        }
    }

    #[test]
    fn test_backfill_rates_empty() {
        let rates = vec![];
        assert_eq!(backfill_rates(rates), vec![]);
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
    fn test_backfill_single_rate() {
        let rate1 = PoolTokenExchangeRate::new_for_testing(100, 100);
        let rates = vec![(1, rate1.clone())];
        let expected = vec![(1, rate1)];
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
