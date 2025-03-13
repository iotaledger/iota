// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { IotaObjectResponse, MoveStruct, MoveValue } from '@iota/iota-sdk/client';
import type { IotaValidatorSummaryExtended } from '../types';

function isMoveStructWithFields(
    data: MoveStruct,
): data is { fields: { [key: string]: MoveValue }; type: string } {
    return (
        typeof data === 'object' &&
        data !== null &&
        'fields' in data &&
        typeof data.fields === 'object' &&
        data.fields !== null
    );
}

function getMoveFields(object: MoveStruct): { [key: string]: MoveValue } {
    if (isMoveStructWithFields(object)) {
        return object.fields as { [key: string]: MoveValue };
    }
    return {};
}

interface MoveStructFields {
    fields: { [key: string]: MoveValue };
}

export function sanitizePendingValidators(
    allPendings: IotaObjectResponse[] | undefined,
): IotaValidatorSummaryExtended[] {
    return (
        allPendings?.map(({ data }) => {
            const fields =
                (data &&
                    data.content &&
                    data.content.dataType === 'moveObject' &&
                    getMoveFields(data.content)) ||
                {} ||
                {};
            const value = fields.value as MoveStructFields;
            const metadata = (value?.fields?.metadata as MoveStructFields)?.fields || {};
            const stakingPool = (value?.fields?.staking_pool as MoveStructFields)?.fields || {};
            const exchangeRates = (stakingPool.exchange_rates as MoveStructFields)?.fields || {};

            return {
                isPending: true,
                authorityPubkeyBytes: '',
                commissionRate: String(value?.fields.commission_rate),
                description: String(metadata.description),
                exchangeRatesId: (
                    exchangeRates.id as {
                        id: string;
                    }
                )?.id,
                exchangeRatesSize: String(exchangeRates.size),
                gasPrice: String(value?.fields.gas_price),
                imageUrl: String(metadata.image_url),
                iotaAddress: String(metadata.iota_address),
                name: String(metadata.name),
                netAddress: String(metadata.net_address),
                networkPubkeyBytes: '',
                nextEpochCommissionRate: String(value?.fields.next_epoch_commission_rate),
                nextEpochGasPrice: String(value?.fields.next_epoch_gas_price),
                nextEpochStake: String(value?.fields.next_epoch_stake),
                operationCapId: String(value?.fields.operation_cap_id),
                p2pAddress: String(metadata.p2p_address),
                pendingPoolTokenWithdraw: String(stakingPool.pending_pool_token_withdraw),
                pendingStake: String(stakingPool.pending_stake),
                pendingTotalIotaWithdraw: String(stakingPool.pending_total_iota_withdraw),
                poolTokenBalance: String(stakingPool.pool_token_balance),
                primaryAddress: String(metadata.primary_address),
                projectUrl: String(metadata.project_url),
                proofOfPossessionBytes: '',
                protocolPubkeyBytes: '',
                rewardsPool: String(stakingPool.rewards_pool),
                stakingPoolId: (
                    stakingPool.id as {
                        id: string;
                    }
                )?.id,
                stakingPoolIotaBalance: String(stakingPool.iota_balance),
                votingPower: String(value?.fields.voting_power),
            };
        }) || []
    );
}
