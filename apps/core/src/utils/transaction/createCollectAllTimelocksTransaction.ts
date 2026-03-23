// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Transaction } from '@iota/iota-sdk/transactions';
import { IOTA_TYPE_ARG, IOTA_FRAMEWORK_ADDRESS, IOTA_CLOCK_OBJECT_ID } from '@iota/iota-sdk/utils';
import type { IotaObjectData } from '@iota/iota-sdk/client';

interface CreateCollectAllTimelocksTransactionOptions {
    address: string;
    timelockObjectIds: string[];
    timelockedStakedObjects: IotaObjectData[];
}

/**
 * Creates a PTB to collect all timelocks and timelock stakes for a user.
 *
 * For timelocks: uses the classic unlock_with_clock approach
 * For timelock stakes:
 *   1. Unlocks them into normal StakedIota using timelocked_staking::unlock_with_clock
 *   2. Joins StakedIota objects that belong to the same pool using staking_pool::join_staked_iota
 */
export function createCollectAllTimelocksTransaction({
    address,
    timelockObjectIds,
    timelockedStakedObjects,
}: CreateCollectAllTimelocksTransactionOptions) {
    const ptb = new Transaction();
    const coins: { $kind: 'NestedResult'; NestedResult: [number, number] }[] = [];

    // 1. Unlock regular timelocks and convert to coins
    for (const objectId of timelockObjectIds) {
        const [unlock] = ptb.moveCall({
            target: `${IOTA_FRAMEWORK_ADDRESS}::timelock::unlock_with_clock`,
            typeArguments: [`${IOTA_FRAMEWORK_ADDRESS}::balance::Balance<${IOTA_TYPE_ARG}>`],
            arguments: [ptb.object(objectId), ptb.object(IOTA_CLOCK_OBJECT_ID)],
        });

        // Convert Balance to Coin
        const [coin] = ptb.moveCall({
            target: `${IOTA_FRAMEWORK_ADDRESS}::coin::from_balance`,
            typeArguments: [IOTA_TYPE_ARG],
            arguments: [ptb.object(unlock)],
        });

        coins.push(coin);
    }

    // 2. Unlock timelock stakes into StakedIota and group by pool
    const stakedIotaByPool = new Map<
        string,
        { $kind: 'NestedResult'; NestedResult: [number, number] }[]
    >();

    for (const stakedObject of timelockedStakedObjects) {
        const objectId = stakedObject.objectId;

        // Get the pool ID from the object content
        const content = stakedObject.content;
        let poolId: string | null = null;
        if (content?.dataType === 'moveObject' && content?.fields) {
            const fields = content.fields as Record<string, unknown>;
            const stakedIotaField = fields.staked_iota;
            if (stakedIotaField && typeof stakedIotaField === 'object') {
                const stakedFields = stakedIotaField as Record<string, unknown>;
                const nestedFields = stakedFields.fields as Record<string, unknown> | undefined;
                poolId = (nestedFields?.pool_id as string) || null;
            }
        }

        // Unlock the timelock staked IOTA using clock
        const [unlockedStakedIota] = ptb.moveCall({
            target: `0x3::timelocked_staking::unlock_with_clock`,
            arguments: [ptb.object(objectId), ptb.object(IOTA_CLOCK_OBJECT_ID)],
        });

        // Group by pool ID for joining
        if (poolId) {
            if (!stakedIotaByPool.has(poolId)) {
                stakedIotaByPool.set(poolId, []);
            }
            stakedIotaByPool.get(poolId)!.push(unlockedStakedIota);
        } else {
            // If we can't determine pool ID, just transfer it
            ptb.transferObjects([unlockedStakedIota], ptb.pure.address(address));
        }
    }

    // 3. Join StakedIota objects that belong to the same pool
    for (const [_poolId, stakedIotaObjects] of stakedIotaByPool.entries()) {
        if (stakedIotaObjects.length === 1) {
            // Only one stake for this pool, just transfer it
            ptb.transferObjects([stakedIotaObjects[0]], ptb.pure.address(address));
        } else {
            // Multiple stakes for this pool, join them
            const [firstStake, ...restStakes] = stakedIotaObjects;

            for (const stake of restStakes) {
                ptb.moveCall({
                    target: `0x3::staking_pool::join_staked_iota`,
                    arguments: [firstStake, stake],
                });
            }

            // Transfer the merged stake
            ptb.transferObjects([firstStake], ptb.pure.address(address));
        }
    }

    // 4. Transfer all collected coins
    if (coins.length > 0) {
        ptb.transferObjects(coins, ptb.pure.address(address));
    }

    return ptb;
}
