// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    SupplyIncreaseVestingPayout,
    SupplyIncreaseVestingPortfolio,
    VestingOverview,
    SupplyIncreaseUserType,
} from '@/lib/interfaces';
import {
    buildSupplyIncreaseVestingSchedule,
    getLatestOrEarliestSupplyIncreaseVestingPayout,
    getVestingOverview,
    isSizeExceededError,
    isSupplyIncreaseVestingObject,
    isTimelockedUnlockable,
    getSupplyIncreaseVestingUserType,
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
    useGetDelegatedStake,
    useIsActiveValidator,
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
    userType: SupplyIncreaseUserType | undefined;
    inactiveValidatorUnlockedStakes: ExtendedDelegatedTimelockedStake[];
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

    const userType = lastPayout ? getSupplyIncreaseVestingUserType([lastPayout]) : undefined;

    // Fetch normal stakes ONLY for Staker users (for join optimization)
    const { data: delegatedStakes } = useGetDelegatedStake({
        address: address || '',
        enabled: !!address && userType === SupplyIncreaseUserType.Staker,
    });

    const supplyIncreaseVestingUnlocked = (() => {
        let filtered = supplyIncreaseVestingMapped?.filter((supplyIncreaseVestingObject) =>
            isTimelockedUnlockable(supplyIncreaseVestingObject, clockTimestampMs),
        );

        if (isMaxTransactionSizeError) {
            filtered = filtered.slice(0, -reductionSize);
        }

        return filtered;
    })();

    const supplyIncreaseVestingUnlockedObjectIds = supplyIncreaseVestingUnlocked.map(
        (unlockedObject) => unlockedObject.id.id,
    );

    const supplyIncreaseVestingUnlockedMaxSize = supplyIncreaseVestingUnlocked.reduce(
        (acc, curr) => (acc += curr.locked.value),
        0n,
    );

    const iotaClient = useIotaClient();

    const { isActiveValidator } = useIsActiveValidator();

    // Get unlocked timelocked staked objects (only for Staker users)
    const availableTimelockedStakes = useMemo(() => {
        if (!timelockedStakedObjects || !clockTimestampMs) return [];

        // Only Stakers can collect timelock stakes - Investors must unstake first
        if (userType !== SupplyIncreaseUserType.Staker) return [];

        return formatDelegatedTimelockedStake(timelockedStakedObjects)
            .filter(isSupplyIncreaseVestingObject)
            .filter((stake) => isTimelockedUnlockable(stake, clockTimestampMs));
    }, [timelockedStakedObjects, clockTimestampMs, userType]);

    // Split by validator status
    const { supplyIncreaseVestingUnlockedStakes, inactiveValidatorUnlockedStakes } = useMemo(() => {
        const timelockedDelegatedStakes: ExtendedDelegatedTimelockedStake[] = [];
        const timelockedStakesDelegatedToInactiveValidator: ExtendedDelegatedTimelockedStake[] = [];

        for (const stake of availableTimelockedStakes) {
            if (isActiveValidator(stake.validatorAddress)) {
                timelockedDelegatedStakes.push(stake);
            } else {
                timelockedStakesDelegatedToInactiveValidator.push(stake);
            }
        }

        return {
            supplyIncreaseVestingUnlockedStakes: timelockedDelegatedStakes,
            inactiveValidatorUnlockedStakes: timelockedStakesDelegatedToInactiveValidator,
        };
    }, [availableTimelockedStakes, isActiveValidator]);

    // Get all timelocked staked object IDs from delegations
    const supplyIncreaseVestingUnlockedStakeObjectData = useMemo(() => {
        return supplyIncreaseVestingUnlockedStakes.map((stake) => ({
            objectId: stake.timelockedStakedIotaId,
            content: {
                dataType: 'moveObject' as const,
                fields: {
                    staked_iota: {
                        fields: {
                            pool_id: stake.stakingPool,
                            stake_activation_epoch: stake.stakeActiveEpoch,
                        },
                    },
                },
            },
        }));
    }, [supplyIncreaseVestingUnlockedStakes]);

    const existingStakedObjects = useMemo(() => {
        if (!delegatedStakes) return [];

        return delegatedStakes.flatMap((delegation) =>
            delegation.stakes
                .filter((stake) => stake.status === 'Active')
                .map((stake) => ({
                    objectId: stake.stakedIotaId,
                    content: {
                        dataType: 'moveObject' as const,
                        fields: {
                            pool_id: delegation.stakingPool,
                            stake_activation_epoch: stake.stakeActiveEpoch,
                        },
                    },
                })),
        );
    }, [delegatedStakes]);

    // Build the collect all transaction
    const unlockAllSupplyIncreaseVesting = useMemo(() => {
        if (
            !address ||
            (supplyIncreaseVestingUnlockedObjectIds.length === 0 &&
                supplyIncreaseVestingUnlockedStakeObjectData.length === 0)
        ) {
            return undefined;
        }

        try {
            const ptb = createCollectAllTimelocksTransaction({
                address,
                timelockObjectIds: supplyIncreaseVestingUnlockedObjectIds,
                timelockedStakedObjects: supplyIncreaseVestingUnlockedStakeObjectData,
                existingStakedObjects: existingStakedObjects,
            });

            ptb.setSenderIfNotSet(address);
            return { transactionBlock: ptb };
        } catch (error) {
            return undefined;
        }
    }, [
        address,
        supplyIncreaseVestingUnlockedObjectIds,
        supplyIncreaseVestingUnlockedStakeObjectData,
        existingStakedObjects,
    ]);

    // Dry run the transaction to check for errors
    const dryRunKey = useMemo(() => {
        const objectIds = supplyIncreaseVestingUnlockedObjectIds.join(',');
        const stakeIds = supplyIncreaseVestingUnlockedStakeObjectData
            .map((s) => s.objectId)
            .join(',');
        const existingIds = existingStakedObjects.map((s) => s.objectId).join(',');
        return `${objectIds}|${stakeIds}|${existingIds}`;
    }, [
        supplyIncreaseVestingUnlockedObjectIds,
        supplyIncreaseVestingUnlockedStakeObjectData,
        existingStakedObjects,
    ]);

    const [isUnlockError, setIsUnlockError] = useState(false);
    const [unlockError, setUnlockError] = useState<Error | null>(null);
    const [isUnlockPending, setIsUnlockPending] = useState(false);

    useEffect(() => {
        let isTransactionAborted = false;

        async function dryRunTransaction() {
            if (!unlockAllSupplyIncreaseVesting?.transactionBlock) {
                setIsUnlockError(false);
                setUnlockError(null);
                setIsUnlockPending(false);
                return;
            }

            setIsUnlockPending(true);
            try {
                const txBytes = await unlockAllSupplyIncreaseVesting.transactionBlock.build({
                    client: iotaClient,
                });
                await iotaClient.dryRunTransactionBlock({ transactionBlock: txBytes });
                if (!isTransactionAborted) {
                    setIsUnlockError(false);
                    setUnlockError(null);
                }
            } catch (error) {
                if (!isTransactionAborted) {
                    setIsUnlockError(true);
                    setUnlockError(error as Error);
                }
            } finally {
                if (!isTransactionAborted) setIsUnlockPending(false);
            }
        }

        dryRunTransaction();
        return () => {
            isTransactionAborted = true;
        };
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [dryRunKey, iotaClient]);

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
        userType,
        inactiveValidatorUnlockedStakes,
    };
}
