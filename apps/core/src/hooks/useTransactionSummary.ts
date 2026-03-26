// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import {
    DryRunTransactionBlockResponse,
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

    const objectIds = objectChanges
        ?.map((change) => 'objectId' in change && change.objectId)
        .filter(Boolean) as string[];

    const { data } = useMultiGetObjects(objectIds, { showDisplay: true });
    const lookup = getObjectDisplayLookup(data);

    const objectChangesWithDisplay = useMemo(
        () =>
            [...(objectChanges ?? [])].map((change) => ({
                ...change,
                display: 'objectId' in change ? lookup?.get(change.objectId) : null,
            })),
        [lookup, objectChanges],
    ) as IotaObjectChangeWithDisplay[];

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
