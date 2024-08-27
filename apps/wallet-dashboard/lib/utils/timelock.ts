// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { DelegatedTimelockedStake, TimelockedStake, IotaObjectData } from '@iota/iota-sdk/client';
import { TimelockedIotaResponse, TimelockedObject } from '../interfaces';

export type ExtendedDelegatedTimelockedStake = TimelockedStake & {
    validatorAddress: string;
    stakingPool: string;
};

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
    currentEpochMs: number,
): boolean {
    return Number(timelockedObject.expirationTimestampMs) <= currentEpochMs;
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

export function formatDelegatedTimelockedStake(
    delegatedTimelockedStakeData: DelegatedTimelockedStake[],
): ExtendedDelegatedTimelockedStake[] {
    return delegatedTimelockedStakeData.flatMap((delegatedTimelockedStake) => {
        return delegatedTimelockedStake.stakes.map((stake) => {
            return {
                validatorAddress: delegatedTimelockedStake.validatorAddress,
                stakingPool: delegatedTimelockedStake.stakingPool,
                estimatedReward: stake.status === 'Active' ? stake.estimatedReward : '',
                stakeActiveEpoch: stake.stakeActiveEpoch,
                stakeRequestEpoch: stake.stakeRequestEpoch,
                status: stake.status,
                timelockedStakedIotaId: stake.timelockedStakedIotaId,
                principal: stake.principal,
                expirationTimestampMs: stake.expirationTimestampMs,
                label: stake.label,
            };
        });
    });
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
