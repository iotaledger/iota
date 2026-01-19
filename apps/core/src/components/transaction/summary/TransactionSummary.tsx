// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type TransactionSummaryType } from '../../..';
import { BalanceChanges, ObjectChanges } from '../../cards';
import { Header, KeyValueInfo, LoadingIndicator, Panel, Title, TitleSize } from '@iota/apps-ui-kit';
import { RenderExplorerLink } from '../../../types';
import { Transaction } from '@iota/iota-sdk/transactions';
import { useEffect, useState } from 'react';

interface TransactionSummaryProps {
    summary: TransactionSummaryType;
    renderExplorerLink: RenderExplorerLink;
    isLoading?: boolean;
    isError?: boolean;
    isDryRun?: boolean;
    transaction?: Transaction;
}

export function TransactionSummary({
    summary,
    isLoading,
    isError,
    isDryRun = false,
    renderExplorerLink,
    transaction,
}: TransactionSummaryProps) {
    const [txHash, setTxHash] = useState<string>('');
    useEffect(() => {
        async function calculateHash() {
            if (transaction) {
                try {
                    const bytes = await transaction.build();
                    const hash = Transaction.getSigningDigest(bytes);
                    setTxHash(hash);
                } catch (error) {
                    console.error('Error building transaction for hash:', error);
                }
            }
        }

        calculateHash();
    }, [transaction]);

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
