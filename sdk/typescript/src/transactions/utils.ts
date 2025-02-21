// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { IotaMoveNormalizedType } from '../client/index.js';
import { normalizeIotaAddress } from '../utils/iota-types.js';
import type { CallArg } from './data/internal.js';

export function extractMutableReference(
    normalizedType: IotaMoveNormalizedType,
): IotaMoveNormalizedType | undefined {
    return typeof normalizedType === 'object' && 'MutableReference' in normalizedType
        ? normalizedType.MutableReference
        : undefined;
}

export function extractReference(
    normalizedType: IotaMoveNormalizedType,
): IotaMoveNormalizedType | undefined {
    return typeof normalizedType === 'object' && 'Reference' in normalizedType
        ? normalizedType.Reference
        : undefined;
}

export function extractStructTag(
    normalizedType: IotaMoveNormalizedType,
): Extract<IotaMoveNormalizedType, { Struct: unknown }> | undefined {
    if (typeof normalizedType === 'object' && 'Struct' in normalizedType) {
        return normalizedType;
    }

    const ref = extractReference(normalizedType);
    const mutRef = extractMutableReference(normalizedType);

    if (typeof ref === 'object' && 'Struct' in ref) {
        return ref;
    }

    if (typeof mutRef === 'object' && 'Struct' in mutRef) {
        return mutRef;
    }
    return undefined;
}

export function getIdFromCallArg(arg: string | CallArg) {
    if (typeof arg === 'string') {
        return normalizeIotaAddress(arg);
    }

    if (arg.Object) {
        if (arg.Object.ImmOrOwnedObject) {
            return normalizeIotaAddress(arg.Object.ImmOrOwnedObject.objectId);
        }

        if (arg.Object.Receiving) {
            return normalizeIotaAddress(arg.Object.Receiving.objectId);
        }

        return normalizeIotaAddress(arg.Object.SharedObject.objectId);
    }

    if (arg.UnresolvedObject) {
        return normalizeIotaAddress(arg.UnresolvedObject.objectId);
    }

    return undefined;
}
