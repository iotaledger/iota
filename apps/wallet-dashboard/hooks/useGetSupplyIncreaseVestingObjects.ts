// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    // useRef,
    useEffect,
    useState,
    useMemo,
} from 'react';
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
    // const limitUnlockObjects = useRef<number | null>(null);
    // const [isDeterminingLimitInProgress, setIsDeterminingInProgress] = useState<boolean>();

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
    const { determinedUnlockedTimelockObjects } = useDetermining(
        supplyIncreaseVestingUnlockedObjectIds,
    );
    const {
        data: unlockAllSupplyIncreaseVesting,
        error,
        isPending,
    } = useUnlockTimelockedObjectsTransaction(address || '', determinedUnlockedTimelockObjects);

    console.log('isPending timelocked', unlockAllSupplyIncreaseVesting, isPending, error);

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

    // useEffect(() => {
    //     if (
    //          &&
    //         limitUnlockObjects.current !== null
    //     ) {
    //         setIsDeterminingInProgress(true);
    //         let nextLimit = limitUnlockObjects.current - 5;
    //         nextLimit = nextLimit > 0 ? nextLimit : 0;

    //         limitUnlockObjects.current = nextLimit;
    //     } else {
    //         console.log('there is no error');
    //     }
    // }, [error?.message, supplyIncreaseVestingUnlocked]);

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

function useDetermining(supplyIncreaseVestingUnlockedObjectIds: string[]) {
    const [maxLimit, setMaxLimit] = useState<number | undefined>();
    const [status, setStatus] = useState<'firstAttempt' | 'idle'>('idle');

    useEffect(() => {
        if (!supplyIncreaseVestingUnlockedObjectIds.length) {
            return;
        }

        setMaxLimit(supplyIncreaseVestingUnlockedObjectIds.length);
        setStatus('firstAttempt');
    }, [supplyIncreaseVestingUnlockedObjectIds]);

    const determinedUnlockedTimelockObjects = useMemo(() => {
        if (status === 'idle' || status === 'firstAttempt') {
            return supplyIncreaseVestingUnlockedObjectIds;
        }
        return supplyIncreaseVestingUnlockedObjectIds.slice(0, maxLimit);
    }, [status, supplyIncreaseVestingUnlockedObjectIds, maxLimit]);

    // console.log('status', status);

    const handleLimitError = (error?: Error) => {
        const hasMessage = error?.message?.includes(
            'Attempting to serialize to BCS, but buffer does not have enough size.',
        );

        if (hasMessage) {
            console.log('--- has message');
        }
    };

    return {
        handleLimitError,
        determinedUnlockedTimelockObjects,
    };
}
