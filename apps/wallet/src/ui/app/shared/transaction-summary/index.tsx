// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { type TransactionSummary as TransactionSummaryType } from '@iota/core';
import clsx from 'clsx';

import { LoadingIndicator } from '_components';
import { Heading } from '../heading';
import { BalanceChanges } from './cards/BalanceChanges';
import { ExplorerLinkCard } from './cards/ExplorerLink';
import { GasSummary } from './cards/GasSummary';
import { ObjectChanges } from './cards/ObjectChanges';

export function TransactionSummary({
    summary,
    isLoading,
    isError,
    isDryRun = false,
    /* todo: remove this, we're using it until we update tx approval page */
    showGasSummary = false,
}: {
    summary: TransactionSummaryType;
    isLoading?: boolean;
    isDryRun?: boolean;
    isError?: boolean;
    showGasSummary?: boolean;
}) {
    if (isError) return null;
    return (
        <section className={clsx('h-full py-lg', { 'py-md': isDryRun })}>
            {isLoading ? (
                <div className="flex items-center justify-center p-10">
                    <LoadingIndicator />
                </div>
            ) : (
                <div className="flex flex-col gap-3">
                    {isDryRun && (
                        <div className="pl-4.5">
                            <Heading variant="heading6" color="steel-darker">
                                Do you approve these actions?
                            </Heading>
                        </div>
                    )}
                    <BalanceChanges changes={summary?.balanceChanges} />
                    <ObjectChanges changes={summary?.objectSummary} />
                    {showGasSummary && <GasSummary gasSummary={summary?.gas} />}
                    <ExplorerLinkCard
                        digest={summary?.digest}
                        timestamp={summary?.timestamp ?? undefined}
                    />
                </div>
            )}
        </section>
    );
}
