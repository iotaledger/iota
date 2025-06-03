// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    ExtendedDelegatedTimelockedStake,
    TimelockedObject,
} from '@iota/core/interfaces/timelock.interfaces';

export type TimelockedStakedObjectsGrouped = {
    validatorAddress: string;
    stakeRequestEpoch: string;
    label: string | null | undefined;
    stakes: ExtendedDelegatedTimelockedStake[];
};

export function isTimelockedStakedIota(
    obj: TimelockedObject | ExtendedDelegatedTimelockedStake,
): obj is ExtendedDelegatedTimelockedStake {
    const referenceProperty: keyof ExtendedDelegatedTimelockedStake = 'timelockedStakedIotaId';
    return referenceProperty in obj;
}

export function isTimelockedObject(
    obj: TimelockedObject | ExtendedDelegatedTimelockedStake,
): obj is TimelockedObject {
    const referenceProperty: keyof TimelockedObject = 'locked';
    return referenceProperty in obj;
}

export function isTimelockedUnlockable(
    timelockedObject: TimelockedObject | ExtendedDelegatedTimelockedStake,
    timestampMs: number,
): boolean {
    return Number(timelockedObject.expirationTimestampMs) <= timestampMs;
}

export function groupTimelockedStakedObjects(
    extendedDelegatedTimelockedStake: ExtendedDelegatedTimelockedStake[],
): TimelockedStakedObjectsGrouped[] {
    const groupedArray: TimelockedStakedObjectsGrouped[] = [];

    extendedDelegatedTimelockedStake.forEach((obj) => {
        let group = groupedArray.find(
            (g) =>
                g.validatorAddress === obj.validatorAddress &&
                g.stakeRequestEpoch === obj.stakeRequestEpoch &&
                g.label === obj.label,
        );

        if (!group) {
            group = {
                validatorAddress: obj.validatorAddress,
                stakeRequestEpoch: obj.stakeRequestEpoch,
                label: obj.label,
                stakes: [],
            };
            groupedArray.push(group);
        }
        group.stakes.push(obj);
    });

    return groupedArray;
}
