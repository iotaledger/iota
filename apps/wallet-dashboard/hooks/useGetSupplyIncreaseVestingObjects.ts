// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

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
    isSizeExceededError,
    isSupplyIncreaseVestingObject,
    isTimelockedUnlockable,
    mapTimelockObjects,
} from '@/lib/utils';
import {
    TIMELOCK_IOTA_TYPE,
    useGetAllOwnedObjects,
    useGetClockTimestamp,
    useGetTimelockedStakedObjects,
    useUnlockTimelockedObjectsTransaction,
} from '@iota/core';
import { Transaction } from '@iota/iota-sdk/transactions';
import { useEffect, useState } from 'react';

const REDUCTION_STEP_SIZE = 5;

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
    isMaxTransactionSizeError: boolean;
    supplyIncreaseVestingUnlockedMaxSize: bigint;
    isUnlockPending: boolean;
    resetMaxTransactionSize: () => void;
    isUnlockError: boolean;
    unlockError: Error | null;
}

export function useGetSupplyIncreaseVestingObjects(address: string): SupplyIncreaseVestingObject {
    const [reductionSize, setReductionSize] = useState(0);
    const [isMaxTransactionSizeError, setIsMaxTransactionSizeError] = useState(false);

    const { data: clockTimestampMs } = useGetClockTimestamp();

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
        clockTimestampMs,
    );

    const nextPayout = getLatestOrEarliestSupplyIncreaseVestingPayout(
        [...supplyIncreaseVestingMapped, ...supplyIncreaseVestingStakedMapped],
        clockTimestampMs,
        false,
    );

    const lastPayout = getLatestOrEarliestSupplyIncreaseVestingPayout(
        [...supplyIncreaseVestingMapped, ...supplyIncreaseVestingStakedMapped],
        clockTimestampMs,
        true,
    );

    const supplyIncreaseVestingPortfolio =
        lastPayout && buildSupplyIncreaseVestingSchedule(lastPayout, clockTimestampMs);

    const supplyIncreaseVestingUnlocked = (() => {
        let filtered = supplyIncreaseVestingMapped?.filter((supplyIncreaseVestingObject) =>
            isTimelockedUnlockable(supplyIncreaseVestingObject, clockTimestampMs),
        );

        if (isMaxTransactionSizeError) {
            filtered = filtered.slice(0, -reductionSize);
        }

        return filtered;
    })();

    const supplyIncreaseVestingUnlockedObjectIds: string[] =
        supplyIncreaseVestingUnlocked.map((unlockedObject) => unlockedObject.id.id) || [];

    const supplyIncreaseVestingUnlockedMaxSize = supplyIncreaseVestingUnlocked.reduce(
        (acc, curr) => (acc += curr.locked.value),
        0n,
    );

    const {
        data: unlockAllSupplyIncreaseVesting,
        isPending: isUnlockPending,
        isError: isUnlockError,
        error: unlockError,
    } = useUnlockTimelockedObjectsTransaction(
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

    function resetMaxTransactionSize() {
        setIsMaxTransactionSizeError(false);
        setReductionSize(0);
    }

    useEffect(() => {
        if (isUnlockError && isSizeExceededError(unlockError)) {
            setIsMaxTransactionSizeError(true);
            setReductionSize((prev) => prev + REDUCTION_STEP_SIZE);
        }
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [isUnlockError, unlockError]);

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
        isMaxTransactionSizeError,
        supplyIncreaseVestingUnlockedMaxSize,
        isUnlockPending,
        resetMaxTransactionSize,
        isUnlockError,
        unlockError,
    };
}
