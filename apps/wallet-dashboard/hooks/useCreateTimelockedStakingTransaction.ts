// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    createTimelockedStakeTransaction,
    TIMELOCK_IOTA_TYPE,
    useGetAllOwnedObjects,
    VestingObject,
} from '@iota/core';
import { useIotaClient } from '@iota/dapp-kit';
import { useQuery } from '@tanstack/react-query';
import { useGetCurrentEpochStartTimestamp } from './useGetCurrentEpochStartTimestamp';
import {
    adjustSplitAmountsInVestingObjects,
    getFormattedTimelockedVestingObjects,
    isSupplyIncreaseVestingObject,
    mapTimelockObjects,
} from '@/lib/utils';
import { TimelockedObject } from '@/lib/interfaces';
import { MIN_STAKING_THRESHOLD } from '@/lib/constants';

export function useCreateTimelockedStakingTransaction(
    validator: string,
    amount: bigint,
    senderAddress: string,
) {
    const { data: currentEpochMs } = useGetCurrentEpochStartTimestamp();
    const { data: timelockedObjects } = useGetAllOwnedObjects(senderAddress, {
        StructType: TIMELOCK_IOTA_TYPE,
    });
    const timelockedMapped = mapTimelockObjects(timelockedObjects || []);

    const filteredTimelockedObjects = timelockedMapped
        ?.filter(isSupplyIncreaseVestingObject)
        .filter((obj: TimelockedObject) => {
            return Number(obj.expirationTimestampMs) > Number(currentEpochMs);
        })
        .sort((a: TimelockedObject, b: TimelockedObject) => {
            return Number(b.expirationTimestampMs) - Number(a.expirationTimestampMs);
        });

    const vestingObjects: VestingObject[] = getFormattedTimelockedVestingObjects(
        filteredTimelockedObjects,
    ).filter((obj) => obj.totalLockedAmount >= MIN_STAKING_THRESHOLD);

    /**
     * Create a subset of objects that meet the stake amount (where total combined locked amount >= STAKE_AMOUNT)
     */
    let totalLocked: bigint = BigInt(0);
    const subsetVestingObjects: VestingObject[] = [];

    for (const obj of vestingObjects) {
        const lockedAmount: bigint = BigInt(obj.totalLockedAmount);
        totalLocked += lockedAmount;
        subsetVestingObjects.push(obj);
        if (totalLocked >= BigInt(amount)) {
            break;
        }
    }

    /**
     * Calculate the remaining amount after staking
     */
    const remainingAmount = totalLocked - amount;
    /**
     * Add splitAmount property to the vesting objects that need to be split
     */
    if (remainingAmount > 0) {
        adjustSplitAmountsInVestingObjects(vestingObjects, remainingAmount);
    }

    const client = useIotaClient();
    return useQuery({
        // eslint-disable-next-line @tanstack/query/exhaustive-deps
        queryKey: ['stake-transaction', validator, amount.toString(), senderAddress],
        queryFn: async () => {
            const transaction = createTimelockedStakeTransaction(vestingObjects, validator);
            transaction.setSender(senderAddress);
            await transaction.build({ client });
            return transaction;
        },
        enabled: !!amount && !!validator && !!senderAddress,
        gcTime: 0,
        select: (transaction) => {
            return {
                transaction,
                gasBudget: transaction.blockData.gasConfig.budget,
            };
        },
    });
}
