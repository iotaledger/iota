// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import {
    DryRunTransactionBlockResponse,
    type IotaObjectChange,
    type IotaTransactionBlockResponse,
} from '@iota/iota-sdk/client';
import { useMemo } from 'react';

import { IotaObjectChangeWithDisplay } from '../types';
import {
    getBalanceChangeSummary,
    getGasSummary,
    getTransactionAction,
    getObjectChangeSummary,
    getObjectDisplayLookup,
    getUnwrappedObjectChangesFromEffects,
} from '../utils';
import { useMultiGetObjects } from './useMultiGetObjects';

export function useTransactionSummary({
    transaction,
    currentAddress,
    recognizedPackagesList,
}: {
    transaction?: IotaTransactionBlockResponse | DryRunTransactionBlockResponse;
    currentAddress?: string;
    recognizedPackagesList: string[];
}) {
    const { objectChanges } = transaction ?? {};

    // Collect object IDs from effects.unwrapped that are missing in objectChanges,
    // so we can fetch their type information.
    const missingUnwrappedIds = useMemo(() => {
        if (!transaction) return [];
        const effects = 'effects' in transaction ? transaction.effects : undefined;
        const unwrappedFromEffects = effects?.unwrapped ?? [];
        if (unwrappedFromEffects.length === 0) return [];

        const existingIds = new Set(
            (objectChanges ?? [])
                .filter((c): c is IotaObjectChange & { objectId: string } => 'objectId' in c)
                .map((c) => c.objectId),
        );

        return unwrappedFromEffects
            .map((entry) => entry.reference.objectId)
            .filter((id) => !existingIds.has(id));
    }, [transaction, objectChanges]);

    const objectIds = [
        ...((objectChanges
            ?.map((change) => 'objectId' in change && change.objectId)
            .filter(Boolean) as string[]) ?? []),
        ...missingUnwrappedIds,
    ];

    const { data } = useMultiGetObjects(objectIds, { showDisplay: true, showType: true });
    const lookup = getObjectDisplayLookup(data);

    // Build a lookup from objectId → objectType for synthesized unwrapped entries.
    const objectTypeLookup = useMemo(() => {
        const map = new Map<string, string>();
        if (!data) return map;
        for (const obj of data) {
            if (obj.data?.objectId && obj.data.type) {
                map.set(obj.data.objectId, obj.data.type);
            }
        }
        return map;
    }, [data]);

    const objectChangesWithDisplay = useMemo(() => {
        const synthesized = transaction
            ? getUnwrappedObjectChangesFromEffects(transaction, objectTypeLookup)
            : [];

        return [...(objectChanges ?? []), ...synthesized].map((change) => ({
            ...change,
            display: 'objectId' in change ? lookup?.get(change.objectId) : null,
        }));
    }, [lookup, objectChanges, transaction, objectTypeLookup]) as IotaObjectChangeWithDisplay[];

    const summary = useMemo(() => {
        if (!transaction) return null;
        const objectSummary = getObjectChangeSummary(objectChangesWithDisplay);
        const balanceChangeSummary = getBalanceChangeSummary(transaction, recognizedPackagesList);
        const gas = getGasSummary(transaction);

        if ('digest' in transaction) {
            // Non-dry-run transaction:
            return {
                gas,
                sender: transaction.transaction?.data.sender,
                balanceChanges: balanceChangeSummary,
                digest: transaction.digest,
                label: getTransactionAction(transaction, currentAddress),
                objectSummary,
                status: transaction.effects?.status.status,
                timestamp: transaction.timestampMs,
                upgradedSystemPackages: transaction.effects?.mutated?.filter(
                    ({ owner }) => owner === 'Immutable',
                ),
            };
        } else {
            // Dry run transaction:
            return {
                gas,
                objectSummary,
                balanceChanges: balanceChangeSummary,
            };
        }
    }, [transaction, objectChangesWithDisplay, recognizedPackagesList, currentAddress]);

    return summary;
}
