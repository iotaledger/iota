// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Transaction } from '@iota/iota-sdk/transactions';
import { IOTA_TYPE_ARG, IOTA_FRAMEWORK_ADDRESS, IOTA_CLOCK_OBJECT_ID } from '@iota/iota-sdk/utils';

// Timelocked stake: fields.staked_iota.fields.{pool_id, stake_activation_epoch}
export interface TimelockedStakeObjectInput {
    objectId: string;
    content: {
        dataType: 'moveObject';
        fields: {
            staked_iota: {
                fields: {
                    pool_id: string;
                    stake_activation_epoch: string;
                };
            };
        };
    };
}

// Regular stake: fields.{pool_id, stake_activation_epoch}
export interface RegularStakeObjectInput {
    objectId: string;
    content: {
        dataType: 'moveObject';
        fields: {
            pool_id: string;
            stake_activation_epoch: string;
        };
    };
}

interface CreateCollectAllTimelocksTransactionOptions {
    address: string;
    timelockObjectIds: string[];
    timelockedStakedObjects: TimelockedStakeObjectInput[];
    existingStakedObjects?: RegularStakeObjectInput[];
}

export function createCollectAllTimelocksTransaction({
    address,
    timelockObjectIds,
    timelockedStakedObjects,
    existingStakedObjects = [],
}: CreateCollectAllTimelocksTransactionOptions) {
    const ptb = new Transaction();
    const coins: { $kind: 'NestedResult'; NestedResult: [number, number] }[] = [];

    // Unlock regular timelocks and convert to coins
    for (const objectId of timelockObjectIds) {
        const [unlock] = ptb.moveCall({
            target: `${IOTA_FRAMEWORK_ADDRESS}::timelock::unlock_with_clock`,
            typeArguments: [`${IOTA_FRAMEWORK_ADDRESS}::balance::Balance<${IOTA_TYPE_ARG}>`],
            arguments: [ptb.object(objectId), ptb.object(IOTA_CLOCK_OBJECT_ID)],
        });

        const [coin] = ptb.moveCall({
            target: `${IOTA_FRAMEWORK_ADDRESS}::coin::from_balance`,
            typeArguments: [IOTA_TYPE_ARG],
            arguments: [ptb.object(unlock)],
        });

        coins.push(coin);
    }

    // Unlock timelock stakes and group by (pool_id, stake_activation_epoch)
    const stakedIotaByKey = new Map<
        string,
        { $kind: 'NestedResult'; NestedResult: [number, number] }[]
    >();

    for (const stakedObject of timelockedStakedObjects) {
        const poolKey = extractPoolKey(stakedObject);

        const [unlockedStakedIota] = ptb.moveCall({
            target: `0x3::timelocked_staking::unlock_with_clock`,
            arguments: [ptb.object(stakedObject.objectId), ptb.object(IOTA_CLOCK_OBJECT_ID)],
        });

        if (poolKey) {
            if (!stakedIotaByKey.has(poolKey)) {
                stakedIotaByKey.set(poolKey, []);
            }
            stakedIotaByKey.get(poolKey)!.push(unlockedStakedIota);
        } else {
            ptb.transferObjects([unlockedStakedIota], ptb.pure.address(address));
        }
    }

    for (const [poolKey, stakedIotaObjects] of stakedIotaByKey.entries()) {
        const existingStake = findExistingStakeForKey(existingStakedObjects, poolKey);

        if (existingStake) {
            joinStakesWithExisting(ptb, existingStake.objectId, stakedIotaObjects);
        } else if (stakedIotaObjects.length === 1) {
            ptb.transferObjects([stakedIotaObjects[0]], ptb.pure.address(address));
        } else {
            const mergedStake = joinMultipleStakes(ptb, stakedIotaObjects);
            ptb.transferObjects([mergedStake], ptb.pure.address(address));
        }
    }

    // Transfer all collected coins
    if (coins.length > 0) {
        ptb.transferObjects(coins, ptb.pure.address(address));
    }

    return ptb;
}

function extractPoolKey(stakedObject: TimelockedStakeObjectInput): string | null {
    const stakedIotaFields = stakedObject.content.fields.staked_iota?.fields;
    if (stakedIotaFields?.pool_id && stakedIotaFields?.stake_activation_epoch) {
        return `${stakedIotaFields.pool_id}:${stakedIotaFields.stake_activation_epoch}`;
    }
    return null;
}

function findExistingStakeForKey(
    existingStakes: RegularStakeObjectInput[],
    poolKey: string,
): RegularStakeObjectInput | undefined {
    return existingStakes.find(
        (s) => `${s.content.fields.pool_id}:${s.content.fields.stake_activation_epoch}` === poolKey,
    );
}

function joinStakesWithExisting(
    ptb: Transaction,
    existingStakeId: string,
    stakes: { $kind: 'NestedResult'; NestedResult: [number, number] }[],
): void {
    const existingStakeObj = ptb.object(existingStakeId);
    for (const stake of stakes) {
        ptb.moveCall({
            target: `0x3::staking_pool::join_staked_iota`,
            arguments: [existingStakeObj, stake],
        });
    }
}

function joinMultipleStakes(
    ptb: Transaction,
    stakes: { $kind: 'NestedResult'; NestedResult: [number, number] }[],
): { $kind: 'NestedResult'; NestedResult: [number, number] } {
    const [firstStake, ...restStakes] = stakes;
    for (const stake of restStakes) {
        ptb.moveCall({
            target: `0x3::staking_pool::join_staked_iota`,
            arguments: [firstStake, stake],
        });
    }
    return firstStake;
}
