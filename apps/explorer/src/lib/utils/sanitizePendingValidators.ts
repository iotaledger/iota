// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type {
    IotaObjectResponse,
    IotaValidatorSummary,
    MoveStruct,
    MoveValue,
} from '@iota/iota-sdk/client';

export type IotaValidatorSummaryExtended = IotaValidatorSummary & { isPending?: boolean };

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
            const metadata =
                ((fields.value as MoveStructFields)?.fields?.metadata as MoveStructFields)
                    ?.fields || {};
            const stakingPool =
                ((fields.value as MoveStructFields)?.fields?.staking_pool as MoveStructFields)
                    ?.fields || {};
            const exchangeRates = (stakingPool.exchange_rates as MoveStructFields)?.fields || {};

            return {
                isPending: true,
                authorityPubkeyBytes: '',
                commissionRate: String((fields.value as MoveStructFields)?.fields.commission_rate),
                description: String(metadata.description),
                exchangeRatesId: (
                    exchangeRates.id as {
                        id: string;
                    }
                )?.id,
                exchangeRatesSize: String(exchangeRates.size),
                gasPrice: String((fields.value as MoveStructFields)?.fields.gas_price),
                imageUrl: String(metadata.image_url),
                iotaAddress: String(metadata.iota_address),
                name: String(metadata.name),
                netAddress: String(metadata.net_address),
                networkPubkeyBytes: '',
                nextEpochCommissionRate: String(
                    (fields.value as MoveStructFields)?.fields.next_epoch_commission_rate,
                ),
                nextEpochGasPrice: String(
                    (fields.value as MoveStructFields)?.fields.next_epoch_gas_price,
                ),
                nextEpochStake: String((fields.value as MoveStructFields)?.fields.next_epoch_stake),
                operationCapId: String((fields.value as MoveStructFields)?.fields.operation_cap_id),
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
                votingPower: String((fields.value as MoveStructFields)?.fields.voting_power),
            };
        }) || []
    );
}
