// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_json_rpc_types::{DelegatedStake, DelegatedTimelockedStake, IotaCommittee, ValidatorApys};
use iota_open_rpc_macros::open_rpc;
use iota_types::{
    base_types::{IotaAddress, ObjectID},
    iota_serde::BigInt,
    iota_system_state::iota_system_state_summary::IotaSystemStateSummary,
};
use jsonrpsee::{core::RpcResult, proc_macros::rpc};

/// The `GovernanceReadApi` trait provides a set of asynchronous methods for
/// accessing governance-related data on the IOTA blockchain. This trait is
/// designed to be used in an RPC context, allowing clients to fetch information
/// about delegated stakes, committees, system state, gas prices, and validator
/// APYs.
///
/// The following methods are available in this trait:
///
/// - `get_stakes_by_ids`: Fetches a list of delegated stakes by their IDs.
/// - `get_stakes`: Retrieves all delegated stakes owned by a specified address.
/// - `get_timelocked_stakes_by_ids`: Fetches a list of delegated timelocked
///   stakes by their IDs.
/// - `get_timelocked_stakes`: Retrieves all delegated timelocked stakes owned
///   by a specified address.
/// - `get_committee_info`: Returns committee information for a specified epoch.
/// - `get_latest_iota_system_state`: Returns a summary of the latest IOTA
///   system state object.
/// - `get_reference_gas_price`: Retrieves the reference gas price for the
///   network.
/// - `get_validators_apy`: Returns the validator annual percentage yield (APY).
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
    async fn get_latest_iota_system_state(&self) -> RpcResult<IotaSystemStateSummary>;

    /// Return the reference gas price for the network
    #[method(name = "getReferenceGasPrice")]
    async fn get_reference_gas_price(&self) -> RpcResult<BigInt<u64>>;

    /// Return the validator APY
    #[method(name = "getValidatorsApy")]
    async fn get_validators_apy(&self) -> RpcResult<ValidatorApys>;
}
