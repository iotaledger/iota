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
    vestingSchedule: VestingOverview;
    vestingPortfolio: SupplyIncreaseVestingPortfolio | undefined;
    vestingMapped: TimelockedObject[];
    vestingStakedMapped: ExtendedDelegatedTimelockedStake[];
    unlockAllTimelockedObjects:
        | {
              transactionBlock: Transaction;
          }
        | undefined;
} {
    const { data: currentEpochMs } = useGetCurrentEpochStartTimestamp();

    const { data: timelockedObjects } = useGetAllOwnedObjects(address || '', {
        StructType: TIMELOCK_IOTA_TYPE,
    });
    const { data: timelockedStakedObjects } = useGetTimelockedStakedObjects(address || '');

    const vestingMapped = mapTimelockObjects(timelockedObjects || []).filter(
        isSupplyIncreaseVestingObject,
    );
    const vestingStakedMapped = formatDelegatedTimelockedStake(
        timelockedStakedObjects || [],
    ).filter(isSupplyIncreaseVestingObject);

    const vestingSchedule = getVestingOverview(
        [...vestingMapped, ...vestingStakedMapped],
        Number(currentEpochMs),
    );

    const nextPayout = getLatestOrEarliestSupplyIncreaseVestingPayout(
        [...vestingMapped, ...vestingStakedMapped],
        Number(currentEpochMs),
        false,
    );

    const lastPayout = getLatestOrEarliestSupplyIncreaseVestingPayout(
        [...vestingMapped, ...vestingStakedMapped],
        Number(currentEpochMs),
        true,
    );

    const vestingPortfolio =
        lastPayout && buildSupplyIncreaseVestingSchedule(lastPayout, Number(currentEpochMs));

    const unlockedTimelockedObjects = vestingMapped?.filter((timelockedObject) =>
        isTimelockedUnlockable(timelockedObject, Number(currentEpochMs)),
    );
    const unlockedTimelockedObjectIds: string[] =
        unlockedTimelockedObjects.map((timelocked) => timelocked.id.id) || [];
    const { data: unlockAllTimelockedObjects } = useUnlockTimelockedObjectsTransaction(
        address || '',
        unlockedTimelockedObjectIds,
    );

    return {
        nextPayout,
        lastPayout,
        vestingSchedule,
        vestingPortfolio,
        vestingMapped,
        vestingStakedMapped,
        unlockAllTimelockedObjects,
    };
}
