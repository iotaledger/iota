// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Transaction } from '@iota/iota-sdk/transactions';
import { IOTA_TYPE_ARG, IOTA_FRAMEWORK_ADDRESS, IOTA_CLOCK_OBJECT_ID } from '@iota/iota-sdk/utils';
import type { IotaObjectData } from '@iota/iota-sdk/client';

interface CreateCollectAllTimelocksTransactionOptions {
    address: string;
    timelockObjectIds: string[];
    timelockedStakedObjects: IotaObjectData[];
    existingStakedObjects?: IotaObjectData[];
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

    // Unlock timelock stakes and group by pool
    const stakedIotaByPool = new Map<
        string,
        { $kind: 'NestedResult'; NestedResult: [number, number] }[]
    >();

    for (const stakedObject of timelockedStakedObjects) {
        const poolId = extractPoolId(stakedObject);

        const [unlockedStakedIota] = ptb.moveCall({
            target: `0x3::timelocked_staking::unlock_with_clock`,
            arguments: [ptb.object(stakedObject.objectId), ptb.object(IOTA_CLOCK_OBJECT_ID)],
        });

        if (poolId) {
            if (!stakedIotaByPool.has(poolId)) {
                stakedIotaByPool.set(poolId, []);
            }
            stakedIotaByPool.get(poolId)!.push(unlockedStakedIota);
        } else {
            ptb.transferObjects([unlockedStakedIota], ptb.pure.address(address));
        }
    }

    // Merge stakes by pool and join with existing stakes
    for (const [poolId, stakedIotaObjects] of stakedIotaByPool.entries()) {
        const existingStake = findExistingStakeForPool(existingStakedObjects, poolId);

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

function extractPoolId(stakedObject: IotaObjectData): string | null {
    const content = stakedObject.content;
    if (content?.dataType === 'moveObject' && content?.fields) {
        const fields = content.fields as Record<string, unknown>;
        const stakedIotaField = fields.staked_iota;
        if (stakedIotaField && typeof stakedIotaField === 'object') {
            const stakedFields = stakedIotaField as Record<string, unknown>;
            const nestedFields = stakedFields.fields as Record<string, unknown> | undefined;
            return (nestedFields?.pool_id as string) || null;
        }
    }
    return null;
}

function findExistingStakeForPool(
    existingStakes: IotaObjectData[],
    poolId: string,
): IotaObjectData | undefined {
    return existingStakes.find((stake) => {
        if (stake.content?.dataType === 'moveObject' && stake.content?.fields) {
            const fields = stake.content.fields as Record<string, unknown>;
            return (fields.pool_id as string) === poolId;
        }
        return false;
    });
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
