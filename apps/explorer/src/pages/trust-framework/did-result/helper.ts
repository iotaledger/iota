// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { IotaObjectData, ObjectOwner } from '@iota/iota-sdk/src/client';
import { type ControllerCap } from './types';

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

/**
 * Gets the owner address from an object owner.
 * @param owner The object owner.
 * @param objectId The object ID, required for 'Shared' owner type.
 * @returns The owner address, or null if not found.
 */
export function getOwnerAddress(owner?: ObjectOwner | null, objectId?: string): string | null {
    if (owner == null) {
        return null;
    }

    switch (getOwnerType(owner)) {
        case 'AddressOwner':
            return getAddressOwner(owner);
        case 'ObjectOwner':
            return getObjectOwner(owner);
        case 'Shared':
            return objectId || null;
        case 'Immutable': // NOTE: What to do with immutable?
        case 'Undefined':
        default:
            return null;
    }
}

/**
 * Gets the type of the object owner.
 * @param owner The object owner.
 * @returns The type of the owner as a string.
 */
export function getOwnerType(owner?: ObjectOwner | null): string {
    if (owner == null) {
        return 'Undefined';
    }

    if (isOwnerType(owner, 'AddressOwner')) {
        return 'AddressOwner';
    }

    if (isOwnerType(owner, 'ObjectOwner')) {
        return 'ObjectOwner';
    }

    if (isOwnerType(owner, 'Shared')) {
        return 'Shared';
    }

    if (isOwnerType(owner, 'Immutable')) {
        return 'Immutable';
    }

    throw new Error(
        'The provided owner do not match a type variation. It is either not an OwnerObject or the type list has grown.',
    );
}

/**
 * A single type helper that handles both object and primitive types in the union.
 */
type KeysOfObjectOrItself<T> = T extends object ? keyof T : T;
type ObjectOwnerKeyVariants = KeysOfObjectOrItself<ObjectOwner>;
type ObjectOwnerVariants = Extract<ObjectOwner, object>;

/**
 * Checks if the owner is of a specific type.
 * @param owner The object owner.
 * @param type The type to check against.
 * @returns True if the owner is of the specified type, false otherwise.
 */
function isOwnerType(
    owner: ObjectOwner,
    type: ObjectOwnerKeyVariants,
): owner is ObjectOwnerVariants {
    return (
        (typeof owner === 'object' && owner !== null && type in owner) ||
        (owner !== null && type === owner)
    );
}

/**
 * Gets the address from an address owner.
 * @param owner The object owner.
 * @returns The address, or null if the owner is not an address owner.
 */
function getAddressOwner(owner: ObjectOwner) {
    if (isAddressOwner(owner)) {
        return owner.AddressOwner;
    }
    return null;
}

/**
 * Gets the object ID from an object owner.
 * @param owner The object owner.
 * @returns The object ID, or null if the owner is not an object owner.
 */
function getObjectOwner(owner: ObjectOwner) {
    if (isObjectOwner(owner)) {
        return owner.ObjectOwner;
    }
    return null;
}

/**
 * Checks if the owner is an address owner.
 * @param owner The object owner.
 * @returns True if the owner is an address owner, false otherwise.
 */
function isAddressOwner(owner: ObjectOwner): owner is { AddressOwner: string } {
    return isOwnerType(owner, 'AddressOwner');
}

/**
 * Checks if the owner is an object owner.
 * @param owner The object owner.
 * @returns True if the owner is an object owner, false otherwise.
 */
function isObjectOwner(owner: ObjectOwner): owner is { ObjectOwner: string } {
    return isOwnerType(owner, 'ObjectOwner');
}
