// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type TransactionSummaryType } from '../../..';
import { BalanceChanges, ObjectChanges } from '../../cards';
import { Header, KeyValueInfo, LoadingIndicator, Panel, Title, TitleSize } from '@iota/apps-ui-kit';
import { RenderExplorerLink } from '../../../types';
import { Transaction } from '@iota/iota-sdk/transactions';
import { useQuery } from '@tanstack/react-query';

interface TransactionSummaryProps {
    summary: TransactionSummaryType;
    renderExplorerLink: RenderExplorerLink;
    isLoading?: boolean;
    isError?: boolean;
    isDryRun?: boolean;
    transaction?: Transaction;
    chain?: string;
}

export function TransactionSummary({
    summary,
    isLoading,
    isError,
    isDryRun = false,
    chain,
    renderExplorerLink,
    transaction,
}: TransactionSummaryProps) {
    const { data: txHash } = useQuery({
        queryKey: ['transaction-signing-digest', transaction?.getData(), transaction],
        async queryFn() {
            if (!transaction) throw new Error('Missing transaction');
            return transaction.getSigningDigest();
        },
    });

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
                    {isDryRun && txHash && (
                        <Panel hasBorder>
                            <div className="flex flex-col overflow-hidden rounded-xl">
                                <Header title="Transaction Hash" />
                                <div className="px-md pb-md">
                                    <KeyValueInfo keyText="" value={txHash} fullwidth />
                                </div>
                            </div>
                        </Panel>
                    )}
                    <BalanceChanges
                        changes={summary?.balanceChanges}
                        renderExplorerLink={renderExplorerLink}
                        chain={chain}
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
