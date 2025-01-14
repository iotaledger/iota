// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type TransactionSummaryType } from '../../..';
import { BalanceChanges, ObjectChanges } from '../../cards';
import { LoadingIndicator, Title, TitleSize } from '@iota/apps-ui-kit';
import { RenderExplorerLink } from '../../../types';

interface TransactionSummaryProps {
    summary: TransactionSummaryType;
    renderExplorerLink: RenderExplorerLink;
    isLoading?: boolean;
    isError?: boolean;
    isDryRun?: boolean;
}

export function TransactionSummary({
    summary,
    isLoading,
    isError,
    isDryRun = false,
    renderExplorerLink,
}: TransactionSummaryProps) {
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
                        <Title title="Do you approve these actions?" size={TitleSize.Medium} />
                    )}
                    <BalanceChanges
                        changes={summary?.balanceChanges}
                        renderExplorerLink={renderExplorerLink}
                    />
                    <ObjectChanges
                        changes={summary?.objectSummary}
                        renderExplorerLink={renderExplorerLink}
                    />
                </div>
            )}
        </>
    );
}
