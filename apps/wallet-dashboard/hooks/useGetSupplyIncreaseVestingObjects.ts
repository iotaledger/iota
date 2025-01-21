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

interface SupplyIncreaseVestingObject {
    nextPayout: SupplyIncreaseVestingPayout | undefined;
    lastPayout: SupplyIncreaseVestingPayout | undefined;
    supplyIncreaseVestingSchedule: VestingOverview;
    supplyIncreaseVestingPortfolio: SupplyIncreaseVestingPortfolio | undefined;
    supplyIncreaseVestingMapped: TimelockedObject[];
    supplyIncreaseVestingStakedMapped: ExtendedDelegatedTimelockedStake[];
    isTimelockedStakedObjectsLoading: boolean;
    unlockAllSupplyIncreaseVesting:
        | {
              transactionBlock: Transaction;
          }
        | undefined;
    refreshStakeList: () => void;
    isSupplyIncreaseVestingScheduleEmpty: boolean;
}

export function useGetSupplyIncreaseVestingObjects(address: string): SupplyIncreaseVestingObject {
    const { data: currentEpochMs } = useGetCurrentEpochStartTimestamp();

    const { data: timelockedObjects, refetch: refetchGetAllOwnedObjects } = useGetAllOwnedObjects(
        address || '',
        {
            StructType: TIMELOCK_IOTA_TYPE,
        },
    );
    const {
        data: timelockedStakedObjects,
        isLoading: isTimelockedStakedObjectsLoading,
        refetch: refetchTimelockedStakedObjects,
    } = useGetTimelockedStakedObjects(address || '');

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
    const { data: unlockAllSupplyIncreaseVesting } = useUnlockTimelockedObjectsTransaction(
        address || '',
        supplyIncreaseVestingUnlockedObjectIds,
    );

    const isSupplyIncreaseVestingScheduleEmpty =
        !supplyIncreaseVestingSchedule.totalVested &&
        !supplyIncreaseVestingSchedule.totalLocked &&
        !supplyIncreaseVestingSchedule.availableClaiming &&
        !supplyIncreaseVestingSchedule.totalStaked &&
        !supplyIncreaseVestingSchedule.totalEarned;

    function refreshStakeList() {
        refetchTimelockedStakedObjects();
        refetchGetAllOwnedObjects();
    }

    return {
        nextPayout,
        lastPayout,
        supplyIncreaseVestingSchedule,
        supplyIncreaseVestingPortfolio,
        supplyIncreaseVestingMapped,
        supplyIncreaseVestingStakedMapped,
        isTimelockedStakedObjectsLoading,
        unlockAllSupplyIncreaseVesting,
        refreshStakeList,
        isSupplyIncreaseVestingScheduleEmpty,
    };
}
