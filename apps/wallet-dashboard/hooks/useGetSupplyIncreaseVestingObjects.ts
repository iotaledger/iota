// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useGetCurrentEpochStartTimestamp } from '@/hooks';
import {
    SupplyIncreaseVestingPayout,
    SupplyIncreaseVestingPortfolio,
    TimelockedObject,
    VestingOverview,
} from '@/lib/interfaces';
import {
    buildSupplyIncreaseVestingSchedule,
    ExtendedDelegatedTimelockedStake,
    formatDelegatedTimelockedStake,
    getLatestOrEarliestSupplyIncreaseVestingPayout,
    getVestingOverview,
    isSupplyIncreaseVestingObject,
    isTimelockedUnlockable,
    mapTimelockObjects,
} from '@/lib/utils';
import {
    TIMELOCK_IOTA_TYPE,
    useGetAllOwnedObjects,
    useGetTimelockedStakedObjects,
    useUnlockTimelockedObjectsTransaction,
} from '@iota/core';
import { Transaction } from '@iota/iota-sdk/transactions';

export function useGetSupplyIncreaseVestingObjects(address: string): {
    nextPayout: SupplyIncreaseVestingPayout | undefined;
    lastPayout: SupplyIncreaseVestingPayout | undefined;
    supplyIncreaseVestingSchedule: VestingOverview;
    supplyIncreaseVestingPortfolio: SupplyIncreaseVestingPortfolio | undefined;
    supplyIncreaseVestingMapped: TimelockedObject[];
    supplyIncreaseVestingStakedMapped: ExtendedDelegatedTimelockedStake[];
    istimelockedStakedObjectsLoading: boolean;
    unlockAllsupplyIncreaseVesting:
        | {
              transactionBlock: Transaction;
          }
        | undefined;
} {
    const { data: currentEpochMs } = useGetCurrentEpochStartTimestamp();

    const { data: timelockedObjects } = useGetAllOwnedObjects(address || '', {
        StructType: TIMELOCK_IOTA_TYPE,
    });
    const { data: timelockedStakedObjects, isPending: istimelockedStakedObjectsLoading } =
        useGetTimelockedStakedObjects(address || '');

    const supplyIncreaseVestingMapped = mapTimelockObjects(timelockedObjects || []).filter(
        isSupplyIncreaseVestingObject,
    );
    const supplyIncreaseVestingStakedMapped = formatDelegatedTimelockedStake(
        timelockedStakedObjects || [],
    ).filter(isSupplyIncreaseVestingObject);

    const supplyIncreaseVestingSchedule = getVestingOverview(
        [...supplyIncreaseVestingMapped, ...supplyIncreaseVestingStakedMapped],
        Number(currentEpochMs),
    );

    const nextPayout = getLatestOrEarliestSupplyIncreaseVestingPayout(
        [...supplyIncreaseVestingMapped, ...supplyIncreaseVestingStakedMapped],
        Number(currentEpochMs),
        false,
    );

    const lastPayout = getLatestOrEarliestSupplyIncreaseVestingPayout(
        [...supplyIncreaseVestingMapped, ...supplyIncreaseVestingStakedMapped],
        Number(currentEpochMs),
        true,
    );

    const supplyIncreaseVestingPortfolio =
        lastPayout && buildSupplyIncreaseVestingSchedule(lastPayout, Number(currentEpochMs));

    const supplyIncreaseVestingUnlocked = supplyIncreaseVestingMapped?.filter(
        (supplyIncreaseVestingObject) =>
            isTimelockedUnlockable(supplyIncreaseVestingObject, Number(currentEpochMs)),
    );
    const supplyIncreaseVestingUnlockedObjectIds: string[] =
        supplyIncreaseVestingUnlocked.map((unlockedObject) => unlockedObject.id.id) || [];
    const { data: unlockAllsupplyIncreaseVesting } = useUnlockTimelockedObjectsTransaction(
        address || '',
        supplyIncreaseVestingUnlockedObjectIds,
    );

    return {
        nextPayout,
        lastPayout,
        supplyIncreaseVestingSchedule,
        supplyIncreaseVestingPortfolio,
        supplyIncreaseVestingMapped,
        supplyIncreaseVestingStakedMapped,
        istimelockedStakedObjectsLoading,
        unlockAllsupplyIncreaseVesting,
    };
}
