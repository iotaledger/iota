// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    SUPPLY_INCREASE_INVESTOR_VESTING_DURATION,
    SUPPLY_INCREASE_STAKER_VESTING_DURATION,
    SUPPLY_INCREASE_STARTING_VESTING_YEAR,
    SUPPLY_INCREASE_VESTING_PAYOUTS_IN_1_YEAR,
    SUPPLY_INCREASE_VESTING_PAYOUT_SCHEDULE,
    VESTING_LABEL,
} from '../constants/vesting.constants';
import {
    SupplyIncreaseUserType,
    SupplyIncreaseVestingPayout,
    SupplyIncreaseVestingPortfolio,
    Timelocked,
    TimelockedStakedIota,
} from '../interfaces';

export function getLastVestingPayout(
    timelocked: Timelocked[],
    timelockedStaked: TimelockedStakedIota[],
): SupplyIncreaseVestingPayout | undefined {
    const vestingObjects = [...timelocked, ...timelockedStaked].filter(
        (obj) => obj.label === VESTING_LABEL,
    );

    if (vestingObjects.length === 0) {
        return undefined;
    }

    const timestampToVestingPayout = new Map<number, SupplyIncreaseVestingPayout>();

    for (const vestingObject of vestingObjects) {
        if (!timestampToVestingPayout.has(vestingObject.expirationTimestampMs)) {
            timestampToVestingPayout.set(vestingObject.expirationTimestampMs, {
                amount: 0,
                expirationTimestampMs: vestingObject.expirationTimestampMs,
            });
        } else {
            const vestingPayout = timestampToVestingPayout.get(vestingObject.expirationTimestampMs);

            if (!vestingPayout) {
                continue;
            }

            if (isTimelocked(vestingObject)) {
                vestingPayout.amount += (vestingObject as Timelocked).locked.value;
            } else if (isTimelockedStakedIota(vestingObject)) {
                vestingPayout.amount += (
                    vestingObject as TimelockedStakedIota
                ).stakedIota.principal.value;
            }

            timestampToVestingPayout.set(vestingObject.expirationTimestampMs, vestingPayout);
        }
    }

    const payouts: SupplyIncreaseVestingPayout[] = Array.from(timestampToVestingPayout.values());

    return payouts.sort((a, b) => b.expirationTimestampMs - a.expirationTimestampMs)[0];
}

function getUserType(payoutTimelocks: number[]): SupplyIncreaseUserType | undefined {
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
    latestPayout: SupplyIncreaseVestingPayout,
): SupplyIncreaseVestingPortfolio {
    const userType = getUserType([latestPayout.expirationTimestampMs]);

    if (!userType || Date.now() >= latestPayout.expirationTimestampMs) {
        // if the latest payout has already been unlocked, we cant build a vesting schedule
        return [];
    }

    const payoutsCount = getVestingPayoutsCount(userType);
    const vestingPortfolio: SupplyIncreaseVestingPortfolio = [];

    for (let i = payoutsCount; i < payoutsCount; i++) {
        vestingPortfolio.push({
            amount: latestPayout.amount,
            expirationTimestampMs:
                latestPayout.expirationTimestampMs - SUPPLY_INCREASE_VESTING_PAYOUT_SCHEDULE * i,
        });
    }
    return vestingPortfolio;
}

// Get number of payouts to construct vesting schedule
function getVestingPayoutsCount(userType: SupplyIncreaseUserType): number {
    const vestingDuration =
        userType === SupplyIncreaseUserType.Staker
            ? SUPPLY_INCREASE_STAKER_VESTING_DURATION
            : SUPPLY_INCREASE_INVESTOR_VESTING_DURATION;

    return SUPPLY_INCREASE_VESTING_PAYOUTS_IN_1_YEAR * vestingDuration;
}

function isTimelockedStakedIota(
    obj: Timelocked | TimelockedStakedIota,
): obj is TimelockedStakedIota {
    return 'stakedIota' in obj;
}

function isTimelocked(obj: Timelocked | TimelockedStakedIota): obj is Timelocked {
    return 'locked' in obj;
}
