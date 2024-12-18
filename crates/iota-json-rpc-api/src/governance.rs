// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use jsonrpsee::core::RpcResult;
use jsonrpsee::proc_macros::rpc;

use iota_json_rpc_types::{DelegatedStake, IotaCommittee, ValidatorApys};
use iota_open_rpc_macros::open_rpc;
use iota_types::base_types::{ObjectID, IotaAddress};
use iota_types::iota_serde::BigInt;
use iota_types::iota_system_state::iota_system_state_summary::IotaSystemStateSummary;

#[open_rpc(namespace = "iotax", tag = "Governance Read API")]
#[rpc(server, client, namespace = "iotax")]
pub trait GovernanceReadApi {
    /// Return one or more [DelegatedStake]. If a Stake was withdrawn its status will be Unstaked.
    #[method(name = "getStakesByIds")]
    async fn get_stakes_by_ids(
        &self,
        staked_iota_ids: Vec<ObjectID>,
    ) -> RpcResult<Vec<DelegatedStake>>;

    /// Return all [DelegatedStake].
    #[method(name = "getStakes")]
    async fn get_stakes(&self, owner: IotaAddress) -> RpcResult<Vec<DelegatedStake>>;

    /// Return the committee information for the asked `epoch`.
    #[method(name = "getCommitteeInfo")]
    async fn get_committee_info(
        &self,
        /// The epoch of interest. If None, default to the latest epoch
        epoch: Option<BigInt<u64>>,
    ) -> RpcResult<IotaCommittee>;

    /// Return the latest IOTA system state object on-chain.
    #[method(name = "getLatestIotaSystemState")]
    async fn get_latest_iota_system_state(&self) -> RpcResult<IotaSystemStateSummary>;

    /// Return the reference gas price for the network
    #[method(name = "getReferenceGasPrice")]
    async fn get_reference_gas_price(&self) -> RpcResult<BigInt<u64>>;

    /// Return the validator APY
    #[method(name = "getValidatorsApy")]
    async fn get_validators_apy(&self) -> RpcResult<ValidatorApys>;
}
