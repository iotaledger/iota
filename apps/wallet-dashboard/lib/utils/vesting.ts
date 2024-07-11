// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    SUPPLY_INCREASE_INVESTOR_VESTING_DURATION,
    SUPPLY_INCREASE_STAKER_VESTING_DURATION,
    SUPPLY_INCREASE_STARTING_VESTING_YEAR,
    SUPPLY_INCREASE_VESTING_PAYOUTS_IN_1_YEAR,
    SUPPLY_INCREASE_VESTING_PAYOUT_SCHEDULE_MILLISECONDS,
    VESTING_LABEL,
} from '../constants/vesting.constants';
import {
    SupplyIncreaseUserType,
    SupplyIncreaseVestingPayout,
    SupplyIncreaseVestingPortfolio,
    Timelocked,
    TimelockedStakedIota,
    VestingOverview,
} from '../interfaces';

export function getLastVestingPayout(
    objects: (Timelocked | TimelockedStakedIota)[],
): SupplyIncreaseVestingPayout | undefined {
    const vestingObjects = objects.filter(isVesting);

    if (vestingObjects.length === 0) {
        return undefined;
    }

    const timestampToVestingPayout = buildExpirationToVestingPayoutMap(vestingObjects);

    const payouts: SupplyIncreaseVestingPayout[] = Array.from(timestampToVestingPayout.values());

    return payouts.sort((a, b) => b.expirationTimestampMs - a.expirationTimestampMs)[0];
}

function buildExpirationToVestingPayoutMap(
    vestingObjects: (Timelocked | TimelockedStakedIota)[],
): Map<number, SupplyIncreaseVestingPayout> {
    const expirationToVestingPayout = new Map<number, SupplyIncreaseVestingPayout>();

    for (const vestingObject of vestingObjects) {
        let objectValue = 0;
        if (isTimelocked(vestingObject)) {
            objectValue = (vestingObject as Timelocked).locked.value;
        } else if (isTimelockedStakedIota(vestingObject)) {
            objectValue = (vestingObject as TimelockedStakedIota).stakedIota.principal.value;
        }

        if (!expirationToVestingPayout.has(vestingObject.expirationTimestampMs)) {
            expirationToVestingPayout.set(vestingObject.expirationTimestampMs, {
                amount: objectValue,
                expirationTimestampMs: vestingObject.expirationTimestampMs,
            });
        } else {
            const vestingPayout = expirationToVestingPayout.get(
                vestingObject.expirationTimestampMs,
            );

            if (!vestingPayout) {
                continue;
            }

            vestingPayout.amount += objectValue;

            expirationToVestingPayout.set(vestingObject.expirationTimestampMs, vestingPayout);
        }
    }

    return expirationToVestingPayout;
}

export function getUserType(payoutTimelocks: number[]): SupplyIncreaseUserType | undefined {
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

export function buildVestingSchedule(
    referencePayout: SupplyIncreaseVestingPayout,
): SupplyIncreaseVestingPortfolio {
    const userType = getUserType([referencePayout.expirationTimestampMs]);

    if (!userType || Date.now() >= referencePayout.expirationTimestampMs) {
        // if the latest payout has already been unlocked, we cant build a vesting schedule
        return [];
    }

    const payoutsCount = getVestingPayoutsCount(userType);

    const vestingPortfolio: SupplyIncreaseVestingPortfolio = [];

    for (let index = 0; index < payoutsCount; index++) {
        vestingPortfolio.push({
            amount: referencePayout.amount,
            expirationTimestampMs:
                referencePayout.expirationTimestampMs -
                SUPPLY_INCREASE_VESTING_PAYOUT_SCHEDULE_MILLISECONDS * index,
        });
    }

    return vestingPortfolio.reverse();
}

export function getVestingOverview(
    objects: (Timelocked | TimelockedStakedIota)[],
): VestingOverview {
    const vestingObjects = objects.filter(isVesting);
    const latestPayout = getLastVestingPayout(vestingObjects);

    if (vestingObjects.length === 0 || !latestPayout) {
        return {
            totalVested: 0,
            totalUnlocked: 0,
            totalLocked: 0,
            totalStacked: 0,
            availableClaiming: 0,
            availableStaking: 0,
        };
    }

    const userType = getUserType([latestPayout.expirationTimestampMs]);
    const vestingPayoutsCount = getVestingPayoutsCount(userType!);
    const totalVestedAmount = vestingPayoutsCount * latestPayout.amount;
    const vestingPortfolio = buildVestingSchedule(latestPayout);
    const totalLockedAmount = vestingPortfolio.reduce(
        (acc, current) => (current.expirationTimestampMs > Date.now() ? acc + current.amount : acc),
        0,
    );
    const totalUnlockedVestedAmount = totalVestedAmount - totalLockedAmount;

    const timelockedStakedObjects = vestingObjects.filter(isTimelockedStakedIota);
    const totalStaked = timelockedStakedObjects.reduce(
        (acc, current) => acc + current.stakedIota.principal.value,
        0,
    );

    const timelockedObjects = vestingObjects.filter(isTimelocked);

    const totalAvailableClaimingAmount = timelockedObjects.reduce(
        (acc, current) =>
            current.expirationTimestampMs <= Date.now() ? acc + current.locked.value : acc,
        0,
    );
    const totalAvailableStakingAmount = timelockedObjects.reduce(
        (acc, current) =>
            current.expirationTimestampMs > Date.now() ? acc + current.locked.value : acc,
        0,
    );

    return {
        totalVested: totalVestedAmount,
        totalUnlocked: totalUnlockedVestedAmount,
        totalLocked: totalLockedAmount,
        totalStacked: totalStaked,
        availableClaiming: totalAvailableClaimingAmount,
        availableStaking: totalAvailableStakingAmount,
    };
}

// Get number of payouts to construct vesting schedule
function getVestingPayoutsCount(userType: SupplyIncreaseUserType): number {
    const vestingDuration =
        userType === SupplyIncreaseUserType.Staker
            ? SUPPLY_INCREASE_STAKER_VESTING_DURATION
            : SUPPLY_INCREASE_INVESTOR_VESTING_DURATION;

    return SUPPLY_INCREASE_VESTING_PAYOUTS_IN_1_YEAR * vestingDuration;
}

export function isTimelockedStakedIota(
    obj: Timelocked | TimelockedStakedIota,
): obj is TimelockedStakedIota {
    const referenceProperty: keyof TimelockedStakedIota = 'stakedIota';
    return referenceProperty in obj;
}

export function isTimelocked(obj: Timelocked | TimelockedStakedIota): obj is Timelocked {
    const referenceProperty: keyof Timelocked = 'locked';
    return referenceProperty in obj;
}

function isVesting(obj: Timelocked | TimelockedStakedIota): boolean {
    return obj.label === VESTING_LABEL;
}
