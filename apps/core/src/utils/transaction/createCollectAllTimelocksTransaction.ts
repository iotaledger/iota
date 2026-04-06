// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Transaction } from '@iota/iota-sdk/transactions';
import {
    IOTA_TYPE_ARG,
    IOTA_FRAMEWORK_ADDRESS,
    IOTA_CLOCK_OBJECT_ID,
    IOTA_SYSTEM_ADDRESS,
} from '@iota/iota-sdk/utils';
import { NestedResultType, RegularStakeObjectInput, TimelockedStakeObjectInput } from '../../types';

interface CreateCollectAllTimelocksTransactionOptions {
    address: string;
    timelockObjectIds: string[];
    timelockedStakedObjects?: TimelockedStakeObjectInput[];
    existingStakedObjects?: RegularStakeObjectInput[];
}

export function createCollectAllTimelocksTransaction({
    address,
    timelockObjectIds,
    timelockedStakedObjects = [],
    existingStakedObjects = [],
}: CreateCollectAllTimelocksTransactionOptions) {
    const ptb = new Transaction();
    const coins: NestedResultType[] = [];

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
    const stakedIotaByKey = new Map<string, NestedResultType[]>();

    for (const stakedObject of timelockedStakedObjects) {
        const [unlockedStakedIota] = ptb.moveCall({
            target: `${IOTA_SYSTEM_ADDRESS}::timelocked_staking::unlock_with_clock`,
            arguments: [ptb.object(stakedObject.objectId), ptb.object(IOTA_CLOCK_OBJECT_ID)],
        });

        const poolKey = extractPoolKey(stakedObject);
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
            // Join all unlocked stakes into the existing regular stake
            for (const stake of stakedIotaObjects) {
                ptb.moveCall({
                    target: `${IOTA_SYSTEM_ADDRESS}::staking_pool::join_staked_iota`,
                    arguments: [ptb.object(existingStake.objectId), stake],
                });
            }
        } else if (stakedIotaObjects.length === 1) {
            ptb.transferObjects([stakedIotaObjects[0]], ptb.pure.address(address));
        } else {
            // Join all into the first one, then transfer
            const [first, ...rest] = stakedIotaObjects;
            for (const stake of rest) {
                ptb.moveCall({
                    target: `${IOTA_SYSTEM_ADDRESS}::staking_pool::join_staked_iota`,
                    arguments: [first, stake],
                });
            }
            ptb.transferObjects([first], ptb.pure.address(address));
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
