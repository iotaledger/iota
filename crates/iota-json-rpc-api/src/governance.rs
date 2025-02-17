// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_json_rpc_types::{DelegatedStake, DelegatedTimelockedStake, IotaCommittee, ValidatorApys};
use iota_open_rpc_macros::open_rpc;
use iota_types::{
    base_types::{IotaAddress, ObjectID},
    iota_serde::BigInt,
    iota_system_state::iota_system_state_summary::{
        IotaSystemStateSummaryV1, IotaSystemStateSummaryV2,
    },
};
use jsonrpsee::{core::RpcResult, proc_macros::rpc};

/// Provides access to validator and staking-related data such as current
/// committee info, delegated stakes, and APY.
#[open_rpc(namespace = "iotax", tag = "Governance Read API")]
#[rpc(server, client, namespace = "iotax")]
pub trait GovernanceReadApi {
    /// Return one or more [DelegatedStake]. If a Stake was withdrawn its status
    /// will be Unstaked.
    #[method(name = "getStakesByIds")]
    async fn get_stakes_by_ids(
        &self,
        staked_iota_ids: Vec<ObjectID>,
    ) -> RpcResult<Vec<DelegatedStake>>;

    /// Return all [DelegatedStake].
    #[method(name = "getStakes")]
    async fn get_stakes(&self, owner: IotaAddress) -> RpcResult<Vec<DelegatedStake>>;

    /// Return one or more [DelegatedTimelockedStake]. If a Stake was withdrawn
    /// its status will be Unstaked.
    #[method(name = "getTimelockedStakesByIds")]
    async fn get_timelocked_stakes_by_ids(
        &self,
        timelocked_staked_iota_ids: Vec<ObjectID>,
    ) -> RpcResult<Vec<DelegatedTimelockedStake>>;

    /// Return all [DelegatedTimelockedStake].
    #[method(name = "getTimelockedStakes")]
    async fn get_timelocked_stakes(
        &self,
        owner: IotaAddress,
    ) -> RpcResult<Vec<DelegatedTimelockedStake>>;

    /// Return the committee information for the asked `epoch`.
    #[method(name = "getCommitteeInfo")]
    async fn get_committee_info(
        &self,
        /// The epoch of interest. If None, default to the latest epoch
        epoch: Option<BigInt<u64>>,
    ) -> RpcResult<IotaCommittee>;

    /// Return the latest IOTA system state object on-chain.
    #[method(name = "getLatestIotaSystemState")]
    async fn get_latest_iota_system_state(&self) -> RpcResult<IotaSystemStateSummaryV2>;

    /// Return the latest IOTA system state object on-chain (version 1).
    /// Requires the `client-target-api-version` header to be set into
    /// a value `<= 0.10` during requests.
    #[method(name = "getLatestIotaSystemState", version <= "0.10")]
    async fn get_latest_iota_system_state_v1(&self) -> RpcResult<IotaSystemStateSummaryV1>;

    /// Return the reference gas price for the network
    #[method(name = "getReferenceGasPrice")]
    async fn get_reference_gas_price(&self) -> RpcResult<BigInt<u64>>;

    /// Return the validator APY
    #[method(name = "getValidatorsApy")]
    async fn get_validators_apy(&self) -> RpcResult<ValidatorApys>;
}
