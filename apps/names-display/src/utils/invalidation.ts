// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

const INVALIDATED_NAMES: Set<string> = new Set(JSON.parse(process.env.INVALIDATED_NAMES || '[]'));

export function isInvalidated(objectId: string | undefined): boolean {
    return objectId !== undefined && INVALIDATED_NAMES.has(objectId);
}
