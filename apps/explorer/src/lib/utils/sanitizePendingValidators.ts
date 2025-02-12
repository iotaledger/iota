// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { IotaObjectResponse, IotaValidatorSummary } from '@iota/iota-sdk/src/client';

export type IotaValidatorSummaryExtended = IotaValidatorSummary & { isPending?: boolean };

export function sanitizePendingValidators(
    allPendings: IotaObjectResponse[] | undefined,
): IotaValidatorSummaryExtended[] {
    return (
        allPendings?.map(({ data }) => {
            // const fieldsData =
            //     data?.content?.dataType === 'moveObject'
            //         ? (data?.content?.fields as Record<string, string | number | object>)
            //         : null;

            const fields = data?.content?.fields?.value?.fields || {};
            const metadata = fields.metadata?.fields || {};
            const stakingPool = fields.staking_pool?.fields || {};
            const exchangeRates = stakingPool.exchange_rates?.fields || {};

            return {
                isPending: true,
                authorityPubkeyBytes: '',
                commissionRate: fields.commission_rate,
                description: metadata.description,
                exchangeRatesId: exchangeRates.id?.id,
                exchangeRatesSize: exchangeRates.size,
                gasPrice: fields.gas_price,
                imageUrl: metadata.image_url,
                iotaAddress: metadata.iota_address,
                name: metadata.name,
                netAddress: metadata.net_address,
                networkPubkeyBytes: '',
                nextEpochAuthorityPubkeyBytes: metadata.next_epoch_authority_pubkey_bytes || null,
                nextEpochCommissionRate: fields.next_epoch_commission_rate,
                nextEpochGasPrice: fields.next_epoch_gas_price,
                nextEpochNetAddress: metadata.next_epoch_net_address || null,
                nextEpochNetworkPubkeyBytes: metadata.next_epoch_network_pubkey_bytes || null,
                nextEpochP2pAddress: metadata.next_epoch_p2p_address || null,
                nextEpochPrimaryAddress: metadata.next_epoch_primary_address || null,
                nextEpochProofOfPossession: metadata.next_epoch_proof_of_possession || null,
                nextEpochProtocolPubkeyBytes: metadata.next_epoch_protocol_pubkey_bytes || null,
                nextEpochStake: fields.next_epoch_stake,
                operationCapId: fields.operation_cap_id,
                p2pAddress: metadata.p2p_address,
                pendingPoolTokenWithdraw: stakingPool.pending_pool_token_withdraw,
                pendingStake: stakingPool.pending_stake,
                pendingTotalIotaWithdraw: stakingPool.pending_total_iota_withdraw,
                poolTokenBalance: stakingPool.pool_token_balance,
                primaryAddress: metadata.primary_address,
                projectUrl: metadata.project_url,
                proofOfPossessionBytes: '',
                protocolPubkeyBytes: '',
                rewardsPool: stakingPool.rewards_pool,
                stakingPoolActivationEpoch: stakingPool.activation_epoch || null,
                stakingPoolDeactivationEpoch: stakingPool.deactivation_epoch || null,
                stakingPoolId: stakingPool.id?.id,
                stakingPoolIotaBalance: stakingPool.iota_balance,
                votingPower: fields.voting_power,
            };
        }) || []
    );
}
