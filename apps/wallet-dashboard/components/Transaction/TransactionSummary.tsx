// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type TransactionSummary as TransactionSummaryType } from '@iota/core';
import { BalanceChanges, ObjectChanges, GasSummary } from './';

export default function TransactionSummary({
    summary,
    isLoading,
    isError,
    showGasSummary = false,
}: {
    summary: TransactionSummaryType;
    isLoading?: boolean;
    isError?: boolean;
    showGasSummary?: boolean;
}) {
    if (isError) return null;
    if (isLoading) return <div>Loading...</div>;
    if (!summary || (!summary.balanceChanges && !summary.objectSummary && !summary.gas))
        return null;

    return (
        <div className="flex flex-col gap-4">
            {summary.balanceChanges && (
                <div className="rounded-md border border-gray-600 p-2">
                    <h4 className="text-center font-semibold">Balance Changes</h4>
                    <BalanceChanges balanceChanges={summary.balanceChanges} />
                </div>
            )}
            {summary.objectSummary && (
                <div className="rounded-md border border-gray-600 p-2">
                    <h4 className="text-center font-semibold">Changes</h4>
                    <ObjectChanges objectSummary={summary.objectSummary} />
                </div>
            )}
            {showGasSummary && summary.gas && <GasSummary gasSummary={summary.gas} />}
        </div>
    );
}
