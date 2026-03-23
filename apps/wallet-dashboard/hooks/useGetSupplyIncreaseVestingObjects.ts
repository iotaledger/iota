// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    SupplyIncreaseVestingPayout,
    SupplyIncreaseVestingPortfolio,
    VestingOverview,
} from '@/lib/interfaces';
import {
    buildSupplyIncreaseVestingSchedule,
    getLatestOrEarliestSupplyIncreaseVestingPayout,
    getVestingOverview,
    isSizeExceededError,
    isSupplyIncreaseVestingObject,
    isTimelockedUnlockable,
} from '@/lib/utils';
import {
    TIMELOCK_IOTA_TYPE,
    useGetAllOwnedObjects,
    useGetClockTimestamp,
    useGetTimelockedStakedObjects,
    TimelockedObject,
    mapTimelockObjects,
    ExtendedDelegatedTimelockedStake,
    formatDelegatedTimelockedStake,
    createCollectAllTimelocksTransaction,
} from '@iota/core';
import { Transaction } from '@iota/iota-sdk/transactions';
import { useEffect, useState, useMemo } from 'react';
import { useIotaClient } from '@iota/dapp-kit';

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
        lastPayout && buildSupplyIncreaseVestingSchedule(lastPayout);

    const supplyIncreaseVestingUnlocked = (() => {
        let filtered = supplyIncreaseVestingMapped?.filter((supplyIncreaseVestingObject) =>
            isTimelockedUnlockable(supplyIncreaseVestingObject, clockTimestampMs),
        );

        if (isMaxTransactionSizeError) {
            filtered = filtered.slice(0, -reductionSize);
        }

        return filtered;
    })();

    const supplyIncreaseVestingUnlockedObjectIds: string[] = useMemo(() => {
        return supplyIncreaseVestingUnlocked.map((unlockedObject) => unlockedObject.id.id) || [];
    }, [supplyIncreaseVestingUnlocked]);

    const supplyIncreaseVestingUnlockedMaxSize = supplyIncreaseVestingUnlocked.reduce(
        (acc, curr) => (acc += curr.locked.value),
        0n,
    );

    const iotaClient = useIotaClient();

    // Get unlocked timelocked staked objects
    const supplyIncreaseVestingUnlockedStakes = useMemo(() => {
        if (!timelockedStakedObjects || !clockTimestampMs) return [];

        return formatDelegatedTimelockedStake(timelockedStakedObjects)
            .filter(isSupplyIncreaseVestingObject)
            .filter((stake) => isTimelockedUnlockable(stake, clockTimestampMs));
    }, [timelockedStakedObjects, clockTimestampMs]);

    // Get all timelocked staked object IDs from delegations
    const supplyIncreaseVestingUnlockedStakeObjectData = useMemo(() => {
        return supplyIncreaseVestingUnlockedStakes.map((stake) => ({
            objectId: stake.timelockedStakedIotaId,
            content: stake,
        }));
    }, [supplyIncreaseVestingUnlockedStakes]);

    // Build the collect all transaction
    const unlockAllSupplyIncreaseVesting = useMemo(() => {
        if (!address || (supplyIncreaseVestingUnlockedObjectIds.length === 0 && supplyIncreaseVestingUnlockedStakeObjectData.length === 0)) {
            return undefined;
        }

        try {
            const ptb = createCollectAllTimelocksTransaction({
                address,
                timelockObjectIds: supplyIncreaseVestingUnlockedObjectIds,
                timelockedStakedObjects: supplyIncreaseVestingUnlockedStakeObjectData as never,
            });

            ptb.setSenderIfNotSet(address);
            return { transactionBlock: ptb };
        } catch (error) {
            return undefined;
        }
    }, [address, supplyIncreaseVestingUnlockedObjectIds, supplyIncreaseVestingUnlockedStakeObjectData]);

    // Dry run the transaction to check for errors
    const [isUnlockError, setIsUnlockError] = useState(false);
    const [unlockError, setUnlockError] = useState<Error | null>(null);
    const [isUnlockPending, setIsUnlockPending] = useState(false);

    useEffect(() => {
        async function dryRunTransaction() {
            if (!unlockAllSupplyIncreaseVesting?.transactionBlock) {
                setIsUnlockError(false);
                setUnlockError(null);
                return;
            }

            setIsUnlockPending(true);
            try {
                await iotaClient.dryRunTransactionBlock({
                    transactionBlock: await unlockAllSupplyIncreaseVesting.transactionBlock.build({ client: iotaClient }),
                });
                setIsUnlockError(false);
                setUnlockError(null);
            } catch (error) {
                setIsUnlockError(true);
                setUnlockError(error as Error);
            } finally {
                setIsUnlockPending(false);
            }
        }

        dryRunTransaction();
    }, [unlockAllSupplyIncreaseVesting, iotaClient]);

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
