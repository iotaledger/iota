// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { IotaObjectData } from '@iota/iota-sdk/src/client';
import { type ControllerCap } from './types';

/**
 * Extracts DID Document from IOTA object data.
 * @param objectData The IOTA object data.
 * @returns a FieldObject or null if deleted.
 */
export function extractDidDoc(objectData: IotaObjectData): FieldObject | null {
    return getField<FieldObject>(objectData.content as FieldObject, 'did_doc') || null;
}

/**
 * Extracts controller capabilities from IOTA object data.
 * @param objectData The IOTA object data.
 * @returns An array of controller capabilities.
 */
export function extractControllerCaps(objectData: IotaObjectData): ControllerCap[] {
    if (objectData.content == null) {
        return [];
    }

    const did_doc = getField<FieldObject>(objectData.content as FieldObject, 'did_doc');
    const controllers = getField<FieldObject>(did_doc, 'controllers');
    const kvControllers = getField<FieldObject[]>(controllers, 'contents');
    const objectControllers = kvControllers.map((fieldObj) => ({
        objectId: getField(fieldObj, 'key') as string,
        weight: Number.parseFloat(getField(fieldObj, 'value')),
    }));
    return objectControllers;
}

/**
 * Extracts the threshold from IOTA object data.
 * @param objectData The IOTA object data.
 * @returns The threshold value as a string, or null if not found.
 */
export function extractThreshold(objectData: IotaObjectData): string | null {
    if (objectData.content == null) {
        return null;
    }

    const did_doc = getField<FieldObject>(objectData.content as FieldObject, 'did_doc');
    const threshold = getField<string | undefined>(did_doc, 'threshold');
    return threshold || null;
}

interface FieldObject {
    fields: { [k: string]: unknown };
    type: string;
}

/**
 * Gets a field from a field object.
 * @template T
 * @param fieldObject The field object.
 * @param field The name of the field to get.
 * @returns The value of the field.
 */
function getField<T>(fieldObject: FieldObject, field: string): T {
    return fieldObject.fields[field] as T;
}
