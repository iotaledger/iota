// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ExtendedTimelockObject } from '@iota/core';
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
    const totalVestedAmount = vestingPayoutsCount * latestPayout.amount;
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
 * Formats an array of timelocked objects into an array of vesting objects.
 * Vesting object is grouped object by expiration time, where objectId holds the id of the first object in the group to which the rest of the objects in the group will be merged.
 *
 * @param timelockedObjects - The array of timelocked objects to be formatted.
 * @returns An array of vesting objects.
 */
export function getFormattedTimelockedObjects(
    timelockedObjects: TimelockedObject[],
): ExtendedTimelockObject[] {
    const expirationMap = new Map<number, TimelockedObject[]>();

    timelockedObjects.forEach((timelockedObject) => {
        const expirationTimestamp = timelockedObject.expirationTimestampMs;

        if (!expirationMap.has(expirationTimestamp)) {
            expirationMap.set(expirationTimestamp, []);
        }
        expirationMap.get(expirationTimestamp)!.push(timelockedObject);
    });

    const result: ExtendedTimelockObject[] = [];

    expirationMap.forEach((objects, expirationTime) => {
        const totalLockedAmount = objects.reduce((sum, obj) => {
            return BigInt(sum) + BigInt(obj.locked.value);
        }, 0n);

        const label = objects[0].label; // Assuming all objects in the group have the same label
        const objectIds = objects.map((obj) => obj.id.id);
        result.push({
            objectId: objectIds[0] || '',
            expirationTimestamp: expirationTime.toString(),
            totalLockedAmount,
            objectIds,
            label,
        });
    });

    return result;
}

/**
 * Adjusts the split amounts in an array of extended timelocked objects based on the total remaining amount.
 * The function iteratively splits the remaining amount among the timelocked objects until the split conditions are met.
 *
 * @param extendedTimelockObjects - An array of extended timelocked objects.
 * @param totalRemainingAmount - The total remaining amount to be split among the extended timelocked objects.
 */
export function adjustSplitAmountsInExtendedTimelockObjects(
    extendedTimelockObjects: ExtendedTimelockObject[],
    totalRemainderAmount: bigint,
) {
    let objectsToSplit = 1;
    let splitAchieved = false;

    while (!splitAchieved && objectsToSplit <= extendedTimelockObjects.length) {
        const baseRemainderAmount = totalRemainderAmount / BigInt(objectsToSplit);
        // if the amount is odd value we need to add the remainder to the first object because we floored the division
        const remainder = totalRemainderAmount % BigInt(objectsToSplit);

        // counter for objects that have splitAmount > 0
        let foundObjectsToSplit = 0;

        for (let i = 0; i < extendedTimelockObjects.length; i++) {
            const obj = extendedTimelockObjects[i];
            let adjustedRemainderAmount = baseRemainderAmount;
            if (i === 0 && remainder > 0) {
                adjustedRemainderAmount += remainder;
            }
            const amountToSplit = obj.totalLockedAmount - adjustedRemainderAmount;

            if (amountToSplit > MIN_STAKING_THRESHOLD) {
                obj.splitAmount = amountToSplit;
                foundObjectsToSplit++;
                if (foundObjectsToSplit === objectsToSplit) {
                    splitAchieved = true;
                    break;
                }
            }
        }

        if (!splitAchieved) {
            extendedTimelockObjects.forEach((obj) => (obj.splitAmount = BigInt(0)));
            objectsToSplit++;
        }
    }
}

/**
 * Prepares vesting objects for timelocked staking.
 *
 * @param timelockedObjects - An array of timelocked objects.
 * @param amount - The amount to stake.
 * @param currentEpochMs - The current epoch in milliseconds.
 * @returns An array of vesting objects that meet the stake amount.
 */
export function prepareObjectsForTimelockedStakingTransaction(
    timelockedObjects: IotaObjectData[],
    amount: bigint,
    currentEpochMs: string,
): ExtendedTimelockObject[] {
    const timelockedMapped = mapTimelockObjects(timelockedObjects);
    const filteredTimelockedObjects = timelockedMapped
        ?.filter(isSupplyIncreaseVestingObject)
        .filter((obj: TimelockedObject) => {
            return Number(obj.expirationTimestampMs) > Number(currentEpochMs);
        })
        .sort((a: TimelockedObject, b: TimelockedObject) => {
            return Number(b.expirationTimestampMs) - Number(a.expirationTimestampMs);
        });

    const extendedTimelockObjects: ExtendedTimelockObject[] = getFormattedTimelockedObjects(
        filteredTimelockedObjects,
    ).filter((obj) => obj.totalLockedAmount >= MIN_STAKING_THRESHOLD);

    /**
     * Create a subset of objects that meet the stake amount (where total combined locked amount >= STAKE_AMOUNT)
     */
    let totalLocked: bigint = BigInt(0);
    const subsetExtendedTimelockObjects: ExtendedTimelockObject[] = [];

    for (const obj of extendedTimelockObjects) {
        totalLocked += obj.totalLockedAmount;
        subsetExtendedTimelockObjects.push(obj);
        if (totalLocked >= amount) {
            break;
        }
    }

    /**
     * Calculate the remaining amount after staking
     */
    const remainingAmount = totalLocked - amount;

    /**
     * Add splitAmount property to the vesting objects that need to be split
     */
    if (remainingAmount > 0) {
        adjustSplitAmountsInExtendedTimelockObjects(subsetExtendedTimelockObjects, remainingAmount);
    }

    return subsetExtendedTimelockObjects;
}
