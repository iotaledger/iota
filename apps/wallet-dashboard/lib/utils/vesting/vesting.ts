// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { GroupedTimelockObject } from '@iota/core';
import {
    MIN_STAKING_THRESHOLD,
    SUPPLY_INCREASE_INVESTOR_VESTING_DURATION,
    SUPPLY_INCREASE_STAKER_VESTING_DURATION,
    SUPPLY_INCREASE_STARTING_VESTING_YEAR,
    SUPPLY_INCREASE_VESTING_LABEL,
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

export function getLastSupplyIncreaseVestingPayout(
    objects: (TimelockedObject | ExtendedDelegatedTimelockedStake)[],
): SupplyIncreaseVestingPayout | undefined {
    const vestingObjects = objects.filter(isSupplyIncreaseVestingObject);

    if (vestingObjects.length === 0) {
        return undefined;
    }

    const vestingPayoutMap = supplyIncreaseVestingObjectsToPayoutMap(vestingObjects);

    const payouts: SupplyIncreaseVestingPayout[] = Array.from(vestingPayoutMap.values());

    return payouts.sort((a, b) => b.expirationTimestampMs - a.expirationTimestampMs)[0];
}

function addVestingPayoutToSupplyIncreaseMap(
    value: number,
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
            const objectValue = Number(vestingObject.principal);
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
    currentEpochTimestamp: number,
): SupplyIncreaseVestingPortfolio {
    const userType = getSupplyIncreaseVestingUserType([referencePayout]);

    if (!userType || currentEpochTimestamp >= referencePayout.expirationTimestampMs) {
        // if the latest payout has already been unlocked, we cant build a vesting schedule
        return [];
    }

    const payoutsCount = getSupplyIncreaseVestingPayoutsCount(userType);

    return Array.from({ length: payoutsCount }).map((_, i) => ({
        amount: referencePayout.amount,
        expirationTimestampMs:
            referencePayout.expirationTimestampMs -
            SUPPLY_INCREASE_VESTING_PAYOUT_SCHEDULE_MILLISECONDS * i,
    }));
}

export function getVestingOverview(
    objects: (TimelockedObject | ExtendedDelegatedTimelockedStake)[],
    currentEpochTimestamp: number,
): VestingOverview {
    const vestingObjects = objects.filter(isSupplyIncreaseVestingObject);
    const latestPayout = getLastSupplyIncreaseVestingPayout(vestingObjects);

    if (vestingObjects.length === 0 || !latestPayout) {
        return {
            totalVested: 0,
            totalUnlocked: 0,
            totalLocked: 0,
            totalStaked: 0,
            availableClaiming: 0,
            availableStaking: 0,
        };
    }

    const userType = getSupplyIncreaseVestingUserType([latestPayout]);
    const vestingPayoutsCount = getSupplyIncreaseVestingPayoutsCount(userType!);
    // note: we add the initial payout to the total rewards, 10% of the total rewards are paid out immediately
    const totalVestedAmount = (vestingPayoutsCount * latestPayout.amount) / 0.9;
    const vestingPortfolio = buildSupplyIncreaseVestingSchedule(
        latestPayout,
        currentEpochTimestamp,
    );
    const totalLockedAmount = vestingPortfolio.reduce(
        (acc, current) =>
            current.expirationTimestampMs > currentEpochTimestamp ? acc + current.amount : acc,
        0,
    );
    const totalUnlockedVestedAmount = totalVestedAmount - totalLockedAmount;

    const timelockedStakedObjects = vestingObjects.filter(isTimelockedStakedIota);
    const totalStaked = timelockedStakedObjects.reduce(
        (acc, current) => acc + Number(current.principal),
        0,
    );

    const timelockedObjects = vestingObjects.filter(isTimelockedObject);

    const totalAvailableClaimingAmount = timelockedObjects.reduce(
        (acc, current) =>
            current.expirationTimestampMs <= currentEpochTimestamp
                ? acc + current.locked.value
                : acc,
        0,
    );
    const totalAvailableStakingAmount = timelockedObjects.reduce(
        (acc, current) =>
            current.expirationTimestampMs > currentEpochTimestamp
                ? acc + current.locked.value
                : acc,
        0,
    );

    return {
        totalVested: totalVestedAmount,
        totalUnlocked: totalUnlockedVestedAmount,
        totalLocked: totalLockedAmount,
        totalStaked: totalStaked,
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
                return BigInt(sum) + BigInt(obj.locked.value);
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
 * @param currentEpochMs - The current epoch in milliseconds.
 * @returns An array of timelocked objects that meet the stake amount.
 */
export function prepareObjectsForTimelockedStakingTransaction(
    timelockedObjects: IotaObjectData[],
    amount: bigint,
    currentEpochMs: string,
): GroupedTimelockObject[] {
    if (Number(amount) === 0) {
        return [];
    }
    const timelockedMapped = mapTimelockObjects(timelockedObjects);
    const filterTimelockedVestingObjects = timelockedMapped
        ?.filter(isSupplyIncreaseVestingObject)
        .filter((obj: TimelockedObject) => {
            return Number(obj.expirationTimestampMs) > Number(currentEpochMs);
        });

    const groupedTimelockObjects: GroupedTimelockObject[] = groupTimelockedObjects(
        filterTimelockedVestingObjects,
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
    let subsetGroupedTimelockObjects: GroupedTimelockObject[] = [];

    for (const groupedObject of groupedTimelockObjects) {
        totalLocked += groupedObject.totalLockedAmount;
        subsetGroupedTimelockObjects.push(groupedObject);
        if (totalLocked >= amount) {
            break;
        }
    }

    // If the total locked amount is less than the stake amount, return not enough locked amount
    if (totalLocked < amount) {
        return [];
    }

    // Calculate the remaining amount after staking
    const remainingAmount = totalLocked - amount;

    // Add splitAmount property to the vesting objects that need to be split
    if (remainingAmount > 0) {
        subsetGroupedTimelockObjects = adjustSplitAmountsInGroupedTimelockObjects(
            subsetGroupedTimelockObjects,
            remainingAmount,
        );
    }

    return subsetGroupedTimelockObjects;
}
