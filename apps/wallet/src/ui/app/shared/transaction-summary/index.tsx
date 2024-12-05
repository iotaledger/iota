// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { type TransactionSummary as TransactionSummaryType } from '@iota/core';
import { BalanceChanges } from './cards/BalanceChanges';
import { ObjectChanges } from './cards/ObjectChanges';
import { LoadingIndicator } from '@iota/apps-ui-kit';

export function TransactionSummary({
    summary,
    isLoading,
    isError,
    isDryRun = false,
}: {
    summary: TransactionSummaryType;
    isLoading?: boolean;
    isDryRun?: boolean;
    isError?: boolean;
}) {
    if (isError) return null;
    return (
        <>
            {isLoading ? (
                <div className="flex items-center justify-center p-10">
                    <LoadingIndicator />
                </div>
            ) : (
                <div className="flex flex-col gap-3">
                    {isDryRun && (
                        <div className="pl-4.5">
                            <span className="text-title-lg text-neutral-10 dark:text-neutral-92">
                                Do you approve these actions?
                            </span>
                        </div>
                    )}
                    <BalanceChanges changes={summary?.balanceChanges} />
                    <ObjectChanges changes={summary?.objectSummary} />
                </div>
            )}
        </>
    );
}
