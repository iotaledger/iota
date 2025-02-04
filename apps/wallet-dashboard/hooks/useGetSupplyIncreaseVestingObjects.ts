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
    SIZE_LIMIT_EXCEEDED,
} from '@iota/core';
import { Transaction } from '@iota/iota-sdk/transactions';
import { useEffect, useMemo, useRef } from 'react';

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
}

export function useGetSupplyIncreaseVestingObjects(address: string): SupplyIncreaseVestingObject {
    const reductionSize = useRef(0);
    const isMaxTransactionSizeError = useRef(false);

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

    const supplyIncreaseVestingUnlocked = useMemo(() => {
        let filtered = supplyIncreaseVestingMapped?.filter((supplyIncreaseVestingObject) =>
            isTimelockedUnlockable(supplyIncreaseVestingObject, Number(currentEpochMs)),
        );

        if (isMaxTransactionSizeError?.current) {
            filtered = filtered.slice(0, -reductionSize.current);
        }

        return filtered;
    }, [supplyIncreaseVestingMapped, currentEpochMs]);

    const supplyIncreaseVestingUnlockedObjectIds: string[] = useMemo(() => {
        const mapped =
            supplyIncreaseVestingUnlocked.map((unlockedObject) => unlockedObject.id.id) || [];

        return mapped;
    }, [supplyIncreaseVestingUnlocked]);

    const supplyIncreaseVestingUnlockedMaxSize = useMemo(() => {
        return supplyIncreaseVestingUnlocked.reduce((acc, curr) => (acc += curr.locked.value), 0n);
    }, [supplyIncreaseVestingUnlocked]);

    const {
        data: unlockAllSupplyIncreaseVesting,
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

    useEffect(() => {
        if (isUnlockError && isSizeExceedError(unlockError)) {
            isMaxTransactionSizeError.current = true;
            reductionSize.current += REDUCTION_STEP_SIZE;
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
        isMaxTransactionSizeError: isMaxTransactionSizeError.current,
        supplyIncreaseVestingUnlockedMaxSize,
    };
}

function isSizeExceedError(e: Error | null) {
    return e?.message?.includes(SIZE_LIMIT_EXCEEDED);
}
