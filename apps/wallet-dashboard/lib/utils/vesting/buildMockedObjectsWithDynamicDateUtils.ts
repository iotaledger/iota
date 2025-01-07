// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { TimelockedObject } from '@/lib/interfaces';
import { DAYS_PER_WEEK, MILLISECONDS_PER_DAY } from '@iota/core/constants/time.constants';
import { DelegatedTimelockedStake } from '@iota/iota-sdk/client';

/**
 * Rebuilds the passed objects to spread the expiration dates from Date.now()
 * to array length with a step of two weeks.
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
