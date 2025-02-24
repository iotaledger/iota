// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { GroupedTimelockObject } from '@iota/core';
import { SUPPLY_INCREASE_VESTING_LABEL } from '@iota/core/constants/vesting.constants';
import {
    MIN_STAKING_THRESHOLD,
    SUPPLY_INCREASE_INVESTOR_VESTING_DURATION,
    SUPPLY_INCREASE_STAKER_VESTING_DURATION,
    SUPPLY_INCREASE_STARTING_VESTING_YEAR,
    SUPPLY_INCREASE_VESTING_PAYOUTS_IN_1_YEAR,
    SUPPLY_INCREASE_VESTING_PAYOUT_SCHEDULE_MILLISECONDS,
} from '../../constants';
import {
    SupplyIncreaseUserType,
    SupplyIncreaseVestingPayout,
    SupplyIncreaseVestingPortfolio,
    TimelockedObject,
    VestingOverview,
} from '../../interfaces';
import {
    ExtendedDelegatedTimelockedStake,
    isTimelockedObject,
    isTimelockedStakedIota,
    mapTimelockObjects,
} from '../timelock';
import { IotaObjectData } from '@iota/iota-sdk/client';

export function getLatestOrEarliestSupplyIncreaseVestingPayout(
    objects: (TimelockedObject | ExtendedDelegatedTimelockedStake)[],
    currentEpochTimestamp: number,
    useLastPayout: boolean = true,
): SupplyIncreaseVestingPayout | undefined {
    const vestingObjects = objects.filter(isSupplyIncreaseVestingObject);

    if (vestingObjects.length === 0) {
        return undefined;
    }

    const vestingPayoutMap = supplyIncreaseVestingObjectsToPayoutMap(vestingObjects);

    let payouts: SupplyIncreaseVestingPayout[] = Array.from(vestingPayoutMap.values());

    if (!useLastPayout) {
        payouts = payouts.filter((payout) => payout.expirationTimestampMs >= currentEpochTimestamp);
    }

    return payouts.sort((a, b) =>
        useLastPayout
            ? b.expirationTimestampMs - a.expirationTimestampMs
            : a.expirationTimestampMs - b.expirationTimestampMs,
    )[0];
}

function addVestingPayoutToSupplyIncreaseMap(
    value: bigint,
    expirationTimestampMs: number,
    supplyIncreaseMap: Map<number, SupplyIncreaseVestingPayout>,
) {
    if (!supplyIncreaseMap.has(expirationTimestampMs)) {
        supplyIncreaseMap.set(expirationTimestampMs, {
            amount: value,
            expirationTimestampMs: expirationTimestampMs,
        });
    } else {
        const vestingPayout = supplyIncreaseMap.get(expirationTimestampMs);
        if (vestingPayout) {
            vestingPayout.amount += value;
            supplyIncreaseMap.set(expirationTimestampMs, vestingPayout);
        }
    }
}

function supplyIncreaseVestingObjectsToPayoutMap(
    vestingObjects: (TimelockedObject | ExtendedDelegatedTimelockedStake)[],
): Map<number, SupplyIncreaseVestingPayout> {
    const expirationToVestingPayout = new Map<number, SupplyIncreaseVestingPayout>();

    for (const vestingObject of vestingObjects) {
        if (isTimelockedObject(vestingObject)) {
            const objectValue = (vestingObject as TimelockedObject).locked.value;
            addVestingPayoutToSupplyIncreaseMap(
                objectValue,
                vestingObject.expirationTimestampMs,
                expirationToVestingPayout,
            );
        } else if (isTimelockedStakedIota(vestingObject)) {
            const objectValue = BigInt(vestingObject.principal);
            const expirationTimestampMs = Number(vestingObject.expirationTimestampMs);
            addVestingPayoutToSupplyIncreaseMap(
                objectValue,
                expirationTimestampMs,
                expirationToVestingPayout,
            );
        }
    }

    return expirationToVestingPayout;
}

export function getSupplyIncreaseVestingUserType(
    vestingUserPayouts: SupplyIncreaseVestingPayout[],
): SupplyIncreaseUserType | undefined {
    const payoutTimelocks = vestingUserPayouts.map((payout) => payout.expirationTimestampMs);
    const latestPayout = payoutTimelocks.sort((a, b) => b - a)[0];

    if (!latestPayout) {
        return;
    } else {
        const isEntity =
            new Date(latestPayout).getFullYear() >
            SUPPLY_INCREASE_STARTING_VESTING_YEAR + SUPPLY_INCREASE_STAKER_VESTING_DURATION;
        return isEntity ? SupplyIncreaseUserType.Entity : SupplyIncreaseUserType.Staker;
    }
}

export function buildSupplyIncreaseVestingSchedule(
    referencePayout: SupplyIncreaseVestingPayout,
    timestampMs: number,
): SupplyIncreaseVestingPortfolio {
    const userType = getSupplyIncreaseVestingUserType([referencePayout]);

    if (!userType || timestampMs >= referencePayout.expirationTimestampMs) {
        // if the latest payout has already been unlocked, we cant build a vesting schedule
        return [];
    }

    const payoutsCount = getSupplyIncreaseVestingPayoutsCount(userType);

    return Array.from({ length: payoutsCount })
        .map((_, i) => ({
            amount: referencePayout.amount,
            expirationTimestampMs:
                referencePayout.expirationTimestampMs -
                SUPPLY_INCREASE_VESTING_PAYOUT_SCHEDULE_MILLISECONDS * i,
        }))
        .sort((a, b) => a.expirationTimestampMs - b.expirationTimestampMs);
}

export function getVestingOverview(
    objects: (TimelockedObject | ExtendedDelegatedTimelockedStake)[],
    timestampMs: number,
): VestingOverview {
    const vestingObjects = objects.filter(isSupplyIncreaseVestingObject);
    const latestPayout = getLatestOrEarliestSupplyIncreaseVestingPayout(
        vestingObjects,
        timestampMs,
    );

    if (vestingObjects.length === 0 || !latestPayout) {
        return {
            totalVested: 0n,
            totalUnlocked: 0n,
            totalLocked: 0n,
            totalStaked: 0n,
            totalEarned: 0n,
            availableClaiming: 0n,
            availableStaking: 0n,
        };
    }

    const userType = getSupplyIncreaseVestingUserType([latestPayout]);
    const vestingPayoutsCount = getSupplyIncreaseVestingPayoutsCount(userType!);
    // Note: we add the initial payout to the total rewards, 10% of the total rewards are paid out immediately
    const totalVestedAmount = (BigInt(vestingPayoutsCount) * latestPayout.amount * 10n) / 9n;
    const vestingPortfolio = buildSupplyIncreaseVestingSchedule(latestPayout, timestampMs);
    const totalLockedAmount = vestingPortfolio.reduce(
        (acc, current) =>
            current.expirationTimestampMs > timestampMs ? acc + BigInt(current.amount) : acc,
        0n,
    );
    const totalUnlockedVestedAmount = totalVestedAmount - totalLockedAmount;

    const timelockedStakedObjects = vestingObjects.filter(isTimelockedStakedIota);
    const totalStaked = timelockedStakedObjects.reduce(
        (acc, current) => acc + BigInt(current.principal),
        0n,
    );

    const totalEarned = timelockedStakedObjects
        .filter((t) => t.status === 'Active')
        .reduce((acc, current) => {
            return acc + BigInt(current.estimatedReward);
        }, 0n);

    const timelockedObjects = vestingObjects.filter(isTimelockedObject);

    const totalAvailableClaimingAmount = timelockedObjects.reduce(
        (acc, current) =>
            current.expirationTimestampMs <= timestampMs ? acc + BigInt(current.locked.value) : acc,
        0n,
    );
    const totalAvailableStakingAmount = timelockedObjects.reduce(
        (acc, current) =>
            current.expirationTimestampMs > timestampMs &&
            current.locked.value >= MIN_STAKING_THRESHOLD
                ? acc + BigInt(current.locked.value)
                : acc,
        0n,
    );

    return {
        totalVested: totalVestedAmount,
        totalUnlocked: totalUnlockedVestedAmount,
        totalLocked: totalLockedAmount,
        totalStaked: totalStaked,
        totalEarned: totalEarned,
        availableClaiming: totalAvailableClaimingAmount,
        availableStaking: totalAvailableStakingAmount,
    };
}

// Get number of payouts to construct vesting schedule
export function getSupplyIncreaseVestingPayoutsCount(userType: SupplyIncreaseUserType): number {
    const vestingDuration =
        userType === SupplyIncreaseUserType.Staker
            ? SUPPLY_INCREASE_STAKER_VESTING_DURATION
            : SUPPLY_INCREASE_INVESTOR_VESTING_DURATION;

    return SUPPLY_INCREASE_VESTING_PAYOUTS_IN_1_YEAR * vestingDuration;
}

export function isSupplyIncreaseVestingObject(
    obj: TimelockedObject | ExtendedDelegatedTimelockedStake,
): boolean {
    return obj.label === SUPPLY_INCREASE_VESTING_LABEL;
}

/**
 * Group an array of timelocked objects into an array of grouped timelocked objects.
 *
 * @param timelockedObjects - The array of timelocked objects to be grouped.
 * @returns An array of grouped timelocked objects.
 */
export function groupTimelockedObjects(
    timelockedObjects: TimelockedObject[],
): GroupedTimelockObject[] {
    const expirationMap = new Map<number, TimelockedObject[]>();

    timelockedObjects.forEach((timelockedObject) => {
        const expirationTimestamp = timelockedObject.expirationTimestampMs;

        if (!expirationMap.has(expirationTimestamp)) {
            expirationMap.set(expirationTimestamp, []);
        }
        expirationMap.get(expirationTimestamp)!.push(timelockedObject);
    });

    const groupedTimelockObjects: GroupedTimelockObject[] = Array.from(
        expirationMap,
        ([expirationTime, objects]) => {
            const totalLockedAmount = objects.reduce((sum, obj) => {
                return sum + BigInt(obj.locked.value);
            }, 0n);

            const label = objects[0].label; // Assuming all objects in the group have the same label
            const objectIds = objects.map((obj) => obj.id.id);

            return {
                objectId: objectIds[0] || '',
                expirationTimestamp: expirationTime.toString(),
                totalLockedAmount,
                mergeObjectIds: objectIds.slice(1),
                label,
            };
        },
    );

    return groupedTimelockObjects;
}

/**
 * Adjusts the split amounts in an array of grouped timelocked objects based on the total remaining amount.
 * The function iteratively splits the remaining amount among the timelocked objects until the split conditions are met.
 *
 * @param groupedTimelockObjects - An array of grouped timelocked objects.
 * @param totalRemainingAmount - The total remaining amount to be split among the grouped timelocked objects.
 */
export function adjustSplitAmountsInGroupedTimelockObjects(
    groupedTimelockObjects: GroupedTimelockObject[],
    totalRemainderAmount: bigint,
): GroupedTimelockObject[] {
    let foundSplit = false;
    let remainderAmount = totalRemainderAmount;

    for (const timelockedObject of groupedTimelockObjects) {
        const amountAvailableToUseAsRemainder =
            timelockedObject.totalLockedAmount - BigInt(MIN_STAKING_THRESHOLD);
        if (amountAvailableToUseAsRemainder <= 0) {
            continue;
        }

        if (amountAvailableToUseAsRemainder >= remainderAmount) {
            timelockedObject.splitAmount = timelockedObject.totalLockedAmount - remainderAmount;
            foundSplit = true;
            break;
        } else {
            remainderAmount -= amountAvailableToUseAsRemainder;
            timelockedObject.splitAmount =
                timelockedObject.totalLockedAmount - amountAvailableToUseAsRemainder;
        }
    }
    if (!foundSplit) {
        return [];
    }
    return groupedTimelockObjects;
}

/**
 * Prepares timelocked objects for timelocked staking.
 *
 * @param timelockedObjects - An array of timelocked objects.
 * @param amount - The amount to stake.
 * @param timestampMs - The current clockTimestamp in milliseconds.
 * @returns An array of timelocked objects that meet the stake amount.
 */
export function prepareObjectsForTimelockedStakingTransaction(
    timelockedObjects: IotaObjectData[],
    targetAmount: bigint,
    timestampMs: number,
): GroupedTimelockObject[] {
    if (targetAmount === 0n) {
        return [];
    }
    const timelockedMapped = mapTimelockObjects(timelockedObjects);
    const stakingEligibleTimelockedObjects = timelockedMapped
        ?.filter(isSupplyIncreaseVestingObject)
        .filter((obj: TimelockedObject) => {
            return Number(obj.expirationTimestampMs) > timestampMs;
        });

    const groupedTimelockObjects: GroupedTimelockObject[] = groupTimelockedObjects(
        stakingEligibleTimelockedObjects,
    )
        .filter((obj) => obj.totalLockedAmount >= MIN_STAKING_THRESHOLD)
        .sort((a: GroupedTimelockObject, b: GroupedTimelockObject) => {
            if (b.totalLockedAmount !== a.totalLockedAmount) {
                return Number(b.totalLockedAmount - a.totalLockedAmount); // Descending order for totalLockedAmount
            }
            return Number(b.expirationTimestamp) - Number(a.expirationTimestamp); // Descending order for expirationTimestamp
        });

    // Create a subset of objects that meet the stake amount (where total combined locked amount >= STAKE_AMOUNT)
    let totalLocked: bigint = BigInt(0);
    let selectedGroupedTimelockObjects: GroupedTimelockObject[] = [];

    for (const groupedObject of groupedTimelockObjects) {
        totalLocked += groupedObject.totalLockedAmount;
        selectedGroupedTimelockObjects.push(groupedObject);
        if (totalLocked >= targetAmount) {
            break;
        }
    }

    // If the total locked amount is less than the stake amount, return not enough locked amount
    if (totalLocked < targetAmount) {
        return [];
    }

    // Calculate the remaining amount after staking
    const remainingAmount = totalLocked - targetAmount;

    // Add splitAmount property to the vesting objects that need to be split
    if (remainingAmount > 0n) {
        selectedGroupedTimelockObjects = adjustSplitAmountsInGroupedTimelockObjects(
            selectedGroupedTimelockObjects,
            remainingAmount,
        );
    }

    return selectedGroupedTimelockObjects;
}
