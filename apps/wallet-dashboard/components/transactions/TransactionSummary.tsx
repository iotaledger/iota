// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type TransactionSummaryType, GasSummary } from '@iota/core';
import { BalanceChanges, ObjectChanges } from '@iota/core';
import { ExplorerLink } from '../ExplorerLink';

interface TransactionSummaryProps {
    summary: TransactionSummaryType;
    activeAddress: string | null | undefined;
    isLoading?: boolean;
    isError?: boolean;
    showGasSummary?: boolean;
}

export function TransactionSummary({
    summary,
    isLoading,
    isError,
    showGasSummary = false,
    activeAddress,
}: TransactionSummaryProps) {
    if (isError) return null;
    if (isLoading) return <div>Loading...</div>;
    if (!summary || (!summary.balanceChanges && !summary.objectSummary && !summary.gas))
        return null;

    return (
        <div className="flex flex-col gap-4">
            {summary.balanceChanges && (
                <div className="rounded-md border border-gray-600 p-2">
                    <h4 className="text-center font-semibold">Balance Changes</h4>
                    <BalanceChanges
                        changes={summary.balanceChanges}
                        renderExplorerLink={ExplorerLink}
                    />
                </div>
            )}
            {summary.objectSummary && (
                <div className="rounded-md border border-gray-600 p-2">
                    <h4 className="text-center font-semibold">Changes</h4>
                    <ObjectChanges
                        changes={summary.objectSummary}
                        renderExplorerLink={ExplorerLink}
                    />
                </div>
            )}
            {showGasSummary && summary.gas && (
                <GasSummary
                    gasSummary={summary.gas}
                    renderExplorerLink={ExplorerLink}
                    activeAddress={activeAddress}
                />
            )}
        </div>
    );
}
