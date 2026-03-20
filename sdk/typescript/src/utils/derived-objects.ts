// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { TypeTag } from '../bcs/bcs.js';
import { TypeTagSerializer } from '../bcs/type-tag-serializer.js';
import { deriveDynamicFieldID } from './dynamic-fields.js';

/**
 * Derive the ID of an object that has been created through `derived_object`.
 */
export function deriveObjectID(
    parentId: string,
    typeTag: typeof TypeTag.$inferInput,
    key: Uint8Array,
) {
    const typeTagStr =
        typeof typeTag === 'string' ? typeTag : TypeTagSerializer.tagToString(typeTag);
    return deriveDynamicFieldID(
        parentId,
        `0x2::derived_object::DerivedObjectKey<${typeTagStr}>`,
        key,
    );
}
