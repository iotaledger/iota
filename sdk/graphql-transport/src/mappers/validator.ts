// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { IotaValidatorSummary } from '@iota/iota-sdk/client';

import type { Rpc_Validator_FieldsFragment } from '../generated/queries.js';

export function mapGraphQlValidatorToRpcValidator(
    validator: Rpc_Validator_FieldsFragment,
): IotaValidatorSummary {
    return {
        commissionRate: validator.commissionRate?.toString()!,
        description: validator.description!,
        exchangeRatesId: validator.exchangeRates?.address!,
        exchangeRatesSize: validator.exchangeRatesSize?.toString()!,
        gasPrice: validator.gasPrice,
        imageUrl: validator.imageUrl!,
        name: validator.name!,
        netAddress: validator.credentials?.netAddress!,
        networkPubkeyBytes: validator.credentials?.networkPubKey!,
        nextEpochCommissionRate: validator.nextEpochCommissionRate?.toString()!,
        nextEpochGasPrice: validator.nextEpochGasPrice,
        nextEpochNetAddress: validator.nextEpochCredentials?.netAddress,
        nextEpochNetworkPubkeyBytes: validator.nextEpochCredentials?.networkPubKey,
        nextEpochP2pAddress: validator.nextEpochCredentials?.p2PAddress,
        nextEpochPrimaryAddress: validator.nextEpochCredentials?.primaryAddress,
        nextEpochProofOfPossession: validator.nextEpochCredentials?.proofOfPossession,
        nextEpochProtocolPubkeyBytes: validator.nextEpochCredentials?.protocolPubKey,
        nextEpochStake: validator.nextEpochStake!,
        nextEpochWorkerAddress: validator.nextEpochCredentials?.workerAddress,
        nextEpochWorkerPubkeyBytes: validator.nextEpochCredentials?.workerPubKey,
        operationCapId: validator.operationCap?.address!,
        p2pAddress: validator.credentials?.p2PAddress!,
        pendingTotalIotaWithdraw: validator.pendingTotalIotaWithdraw,
        pendingPoolTokenWithdraw: validator.pendingPoolTokenWithdraw,
        poolTokenBalance: validator.poolTokenBalance,
        pendingStake: validator.pendingStake,
        primaryAddress: validator.credentials?.primaryAddress!,
        projectUrl: validator.projectUrl!,
        proofOfPossessionBytes: validator.credentials?.proofOfPossession,
        protocolPubkeyBytes: validator.credentials?.protocolPubKey,
        rewardsPool: validator.rewardsPool,
        stakingPoolId: validator.stakingPool?.address!,
        stakingPoolActivationEpoch: validator.stakingPoolActivationEpoch?.toString(),
        stakingPoolIotaBalance: validator.stakingPoolIotaBalance,
        iotaAddress: validator.address.address,
        votingPower: validator.votingPower?.toString()!,
        workerAddress: validator.credentials?.workerAddress!,
        workerPubkeyBytes: validator.credentials?.workerPubKey,
    };
}
