// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { DelegatedTimelockedStake, IotaObjectData } from '@iota/iota-sdk/client';
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
    TimelockedIotaResponse,
    TimelockedObject,
    VestingOverview,
} from '../../interfaces';
import { isTimelockedObject, isTimelockedStakedIota } from '../timelock';
import { VestingObject } from '@iota/core';

export function getLastSupplyIncreaseVestingPayout(
    objects: (TimelockedObject | DelegatedTimelockedStake)[],
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
    vestingObjects: (TimelockedObject | DelegatedTimelockedStake)[],
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
            for (const vestingStake of vestingObject.stakes) {
                const objectValue = Number(vestingStake.principal);
                const expirationTimestampMs = Number(vestingStake.expirationTimestampMs);
                addVestingPayoutToSupplyIncreaseMap(
                    objectValue,
                    expirationTimestampMs,
                    expirationToVestingPayout,
                );
            }
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
    objects: (TimelockedObject | DelegatedTimelockedStake)[],
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
    let totalStaked: number = 0;
    for (const timelockedStakedObject of timelockedStakedObjects) {
        const stakesAmount = timelockedStakedObject.stakes.reduce(
            (acc, current) => acc + Number(current.principal),
            0,
        );
        totalStaked += stakesAmount;
    }

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

export function mapTimelockObjects(iotaObjects: IotaObjectData[]): TimelockedObject[] {
    return iotaObjects.map((iotaObject) => {
        if (!iotaObject?.content?.dataType || iotaObject.content.dataType !== 'moveObject') {
            return {
                id: { id: '' },
                locked: { value: 0 },
                expirationTimestampMs: 0,
            };
        }
        const fields = iotaObject.content.fields as unknown as TimelockedIotaResponse;
        return {
            id: fields.id,
            locked: { value: Number(fields.locked) },
            expirationTimestampMs: Number(fields.expiration_timestamp_ms),
            label: fields.label,
        };
    });
}

export function isSupplyIncreaseVestingObject(
    obj: TimelockedObject | DelegatedTimelockedStake,
): boolean {
    if (isTimelockedObject(obj)) {
        return obj.label === SUPPLY_INCREASE_VESTING_LABEL;
    } else if (isTimelockedStakedIota(obj)) {
        return obj.stakes.some((stake) => stake.label === SUPPLY_INCREASE_VESTING_LABEL);
    } else {
        return false;
    }
}

/**
 * Create VestingObject array from IotaObjectResponse array
 * Vesting object is grouped object by expiration time, where objectId holds the id of the first object in the group to which the rest of the objects in the group will be merged.
 * It also holds the total locked amount of the group.
 */
export function getFormattedTimelockedVestingObjects(
    timelockedObjects: TimelockedObject[],
): VestingObject[] {
    const expirationMap = new Map<number, TimelockedObject[]>();

    timelockedObjects.forEach((timelockedObject) => {
        const expirationTimestamp = timelockedObject.expirationTimestampMs;

        if (!expirationMap.has(expirationTimestamp)) {
            expirationMap.set(expirationTimestamp, []);
        }
        expirationMap.get(expirationTimestamp)!.push(timelockedObject);
    });

    const result: VestingObject[] = [];

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
 * Adjust split amounts in vesting objects to ensure that the stake amout is met and that the split amount is greater than the minimum staking threshold
 */
export function adjustSplitAmountsInVestingObjects(
    vestingObjects: VestingObject[],
    totalRemainingAmount: bigint,
) {
    let objectsToSplit = 1;
    let splitAchieved = false;

    while (!splitAchieved && objectsToSplit <= vestingObjects.length) {
        const splitRemainingAmount = BigInt(
            Math.floor(Number(totalRemainingAmount) / objectsToSplit),
        );
        // if the amount is odd value we need to add the remainder to the first object because we floored the division
        const splitRemainder = BigInt(totalRemainingAmount) % BigInt(objectsToSplit);
        // counter for objects that have splitAmount > 0
        let foundObjectsToSplit = 0;

        // Reset splitAmounts to 0
        vestingObjects.forEach((obj) => (obj.splitAmount = BigInt(0)));

        for (let i = 0; i < vestingObjects.length; i++) {
            const obj = vestingObjects[i];
            let remainingAmount = splitRemainingAmount;
            if (i === 0 && splitRemainder > 0) {
                remainingAmount += splitRemainder;
            }
            const amountToSplit = BigInt(obj.totalLockedAmount) - remainingAmount;

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
            objectsToSplit++;
        }
    }
}

export function prepareVestingObjectsForTimelockedStaking(
    timelockedObjects: IotaObjectData[],
    amount: bigint,
    currentEpochMs: string,
) {
    const timelockedMapped = mapTimelockObjects(timelockedObjects || []);
    const filteredTimelockedObjects = timelockedMapped
        ?.filter(isSupplyIncreaseVestingObject)
        .filter((obj: TimelockedObject) => {
            return Number(obj.expirationTimestampMs) > Number(currentEpochMs);
        })
        .sort((a: TimelockedObject, b: TimelockedObject) => {
            return Number(b.expirationTimestampMs) - Number(a.expirationTimestampMs);
        });

    const vestingObjects: VestingObject[] = getFormattedTimelockedVestingObjects(
        filteredTimelockedObjects,
    ).filter((obj) => obj.totalLockedAmount >= MIN_STAKING_THRESHOLD);

    /**
     * Create a subset of objects that meet the stake amount (where total combined locked amount >= STAKE_AMOUNT)
     */
    let totalLocked: bigint = BigInt(0);
    const subsetVestingObjects: VestingObject[] = [];

    for (const obj of vestingObjects) {
        totalLocked += obj.totalLockedAmount;
        subsetVestingObjects.push(obj);
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
        adjustSplitAmountsInVestingObjects(subsetVestingObjects, remainingAmount);
    }

    return subsetVestingObjects;
}
