// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type TransactionSummaryType } from '../../..';
import { BalanceChanges, ObjectChanges } from '../../cards';
import { Header, KeyValueInfo, LoadingIndicator, Panel, Title, TitleSize } from '@iota/apps-ui-kit';
import { RenderExplorerLink } from '../../../types';
import { toHex, fromBase64 } from '@iota/iota-sdk/utils';
import { signingDigest } from '@iota/iota-sdk/cryptography';
import { useMemo } from 'react';

interface TransactionSummaryProps {
    summary: TransactionSummaryType;
    renderExplorerLink: RenderExplorerLink;
    isLoading?: boolean;
    isError?: boolean;
    isDryRun?: boolean;
    transactionBytes?: string;
}

export function TransactionSummary({
    summary,
    isLoading,
    isError,
    isDryRun = false,
    renderExplorerLink,
    transactionBytes,
}: TransactionSummaryProps) {
    const txHash = useMemo(() => {
        if (transactionBytes) {
            try {
                const bytes = fromBase64(transactionBytes);
                const digest = signingDigest(bytes, 'TransactionData');
                return '0x' + toHex(digest);
            } catch (error) {
                console.error('Error calculating transaction hash:', error);
                return '';
            }
        }
        return '';
    }, [transactionBytes]);

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
