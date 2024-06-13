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
    return (
        <div>
            {isLoading ? (
                <div>Loading...</div>
            ) : (
                <div className="flex flex-col gap-4">
                    <BalanceChanges balanceChanges={summary?.balanceChanges} />
                    <ObjectChanges objectSummary={summary?.objectSummary} />
                    {showGasSummary && <GasSummary gasSummary={summary?.gas} />}
                </div>
            )}
        </div>
    );
}
