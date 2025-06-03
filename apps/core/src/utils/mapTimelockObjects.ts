// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IotaObjectData } from '@iota/iota-sdk/client';
import { TimelockedIotaObjectSchema, TimelockedObjectFieldsSchema } from './stake/types';
import { TimelockedIotaResponse, TimelockedObject } from '../interfaces/timelock.interfaces';

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
