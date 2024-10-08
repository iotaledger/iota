// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Struct } from 'superstruct';
import { create as superstructCreate } from 'superstruct';

import type { IotaMoveNormalizedType } from '../client/index.js';

export function create<T, S>(value: T, struct: Struct<T, S>): T {
    return superstructCreate(value, struct);
}

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
