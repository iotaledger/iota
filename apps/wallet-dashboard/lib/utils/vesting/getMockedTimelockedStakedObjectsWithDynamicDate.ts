// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { MILLISECONDS_PER_DAY } from '../../constants';
import { DelegatedTimelockedStake } from '@iota/iota-sdk/client';

/**
 * Gets the objects in a distributed manner with half of the objects
 * being unlocked and the other half being locked.
 */
export function getMockedTimelockedStakedObjectsWithDynamicDate(
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
