// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    DryRunTransactionBlockResponse,
    type IotaObjectChange,
    type IotaTransactionBlockResponse,
} from '@iota/iota-sdk/client';

/**
 * TODO: Remove this helper once unwrapped objects in objectChanges by default.
 */
export function getUnwrappedObjectChangesFromEffects(
    transaction: IotaTransactionBlockResponse | DryRunTransactionBlockResponse,
    objectTypeLookup: Map<string, string>,
): IotaObjectChange[] {
    const effects = 'effects' in transaction ? transaction.effects : undefined;
    const existingObjectChanges = transaction.objectChanges ?? [];

    const unwrappedFromEffects = effects?.unwrapped ?? [];
    if (unwrappedFromEffects.length === 0) return [];

    const existingIds = new Set(
        existingObjectChanges
            .filter((c): c is IotaObjectChange & { objectId: string } => 'objectId' in c)
            .map((c) => c.objectId),
    );

    const sender =
        'transaction' in transaction
            ? transaction.transaction?.data.sender
            : (transaction as DryRunTransactionBlockResponse).input?.sender;

    return unwrappedFromEffects
        .filter((entry) => !existingIds.has(entry.reference.objectId))
        .map((entry) => ({
            type: 'unwrapped' as const,
            objectId: entry.reference.objectId,
            digest: entry.reference.digest,
            version: entry.reference.version,
            owner: entry.owner,
            sender: sender ?? '',
            objectType: objectTypeLookup.get(entry.reference.objectId) ?? '',
        }));
}
