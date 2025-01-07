// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { TimelockedObject } from '@/lib/interfaces';
import { DAYS_PER_WEEK, MILLISECONDS_PER_DAY } from '@iota/core/constants/time.constants';
import { DelegatedTimelockedStake } from '@iota/iota-sdk/client';

/**
 * Maps the passed objects to a set of objects with modified expirationTimestampMs date.
 * It spreads the dates from CURRENT datetime PLUS two weeks,
 * so that the reference payout we use will always be with expiration of two weeks from now,
 * then the rest are spread BACK up to array length with a STEP of two weeks (vesting schedule).
 */
export function getMockedSupplyIncreaseVestingTimelockedObjectsWithDynamicDate(
    vestingObjects: TimelockedObject[],
): TimelockedObject[] {
    const twoWeeksMs = 2 * DAYS_PER_WEEK * MILLISECONDS_PER_DAY;
    const twoWeeksFromNow = Date.now() + twoWeeksMs;

    return structuredClone(vestingObjects)
        .map((object, idx) => {
            object.expirationTimestampMs = twoWeeksFromNow - idx * twoWeeksMs;
            return object;
        })
        .reverse();
}

/**
 * Gets the objects in a distributed manner with half of the objects
 * being unlocked and the other half being locked.
 */
export function getMockedVestingTimelockedStakedObjectsWithDynamicDate(
    delegatedObjects: DelegatedTimelockedStake[],
): DelegatedTimelockedStake[] {
    const now = Date.now();
    const fourteenDaysMs = 14 * MILLISECONDS_PER_DAY;

    return structuredClone(delegatedObjects).map((object) => {
        const halfLength = Math.ceil(object.stakes.length / 2);
        const leftHalf = object.stakes.slice(0, halfLength);
        const rightHalf = object.stakes.slice(halfLength);

        for (let index = leftHalf.length - 1; index >= 0; index--) {
            const stake = leftHalf[index];

            stake.expirationTimestampMs = (now - (index + 1) * fourteenDaysMs).toString();
        }

        for (let index = 0; index < rightHalf.length; index++) {
            const stake = rightHalf[index];

            stake.expirationTimestampMs = (now + (index + 1) * fourteenDaysMs).toString();
        }

        return { ...object, stakes: [...leftHalf, ...rightHalf] };
    });
}
