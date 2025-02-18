// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { DelegatedTimelockedStake, TimelockedStake, IotaObjectData } from '@iota/iota-sdk/client';
import { TimelockedIotaResponse, TimelockedObject } from '../interfaces';
import {
    TimelockedIotaObjectSchema,
    TimelockedObjectFieldsSchema,
    DelegatedTimelockedStakeSchema,
} from '@iota/core/utils/stake/types';

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
    timestampMs: number,
): boolean {
    return Number(timelockedObject.expirationTimestampMs) <= timestampMs;
}

export function mapTimelockObjects(iotaObjects: IotaObjectData[]): TimelockedObject[] {
    return iotaObjects.map((iotaObject) => {
        const validIotaObject = TimelockedIotaObjectSchema.parse(iotaObject);

        if (
            !validIotaObject?.content?.dataType ||
            validIotaObject.content.dataType !== 'moveObject'
        ) {
            return {
                id: { id: '' },
                locked: { value: 0n },
                expirationTimestampMs: 0,
            };
        }
        const fields = validIotaObject.content.fields as unknown as TimelockedIotaResponse;

        const validFields = TimelockedObjectFieldsSchema.parse(fields);

        return {
            id: validFields.id,
            locked: { value: BigInt(validFields?.locked || '0') },
            expirationTimestampMs: Number(validFields.expiration_timestamp_ms),
            label: validFields.label,
        };
    });
}

export function formatDelegatedTimelockedStake(
    delegatedTimelockedStakeData: DelegatedTimelockedStake[],
): ExtendedDelegatedTimelockedStake[] {
    return delegatedTimelockedStakeData.flatMap((delegatedTimelockedStake) => {
        const validatedDelegatedTimelockedStake =
            DelegatedTimelockedStakeSchema.parse(delegatedTimelockedStake);

        return validatedDelegatedTimelockedStake.stakes.map((stake) => {
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
