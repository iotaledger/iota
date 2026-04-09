// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ExplorerLink } from '@/components';
import { Header, LoadingIndicator, InfoBox, InfoBoxType, InfoBoxStyle } from '@iota/apps-ui-kit';
import { Warning } from '@iota/apps-ui-icons';
import {
    useTransactionSummary,
    useGetTransaction,
    ViewTxnOnExplorerButton,
    ExplorerLinkType,
    TransactionReceipt,
    useRecognizedPackages,
    OutlinedCopyButton,
} from '@iota/core';
import { useCurrentAccount } from '@iota/dapp-kit';
import { IotaTransactionBlockResponse } from '@iota/iota-sdk/client';
import { DialogLayoutBody, DialogLayoutFooter } from '../layout';
import { trackElementCopied } from '@/lib/utils';
import { useCallback } from 'react';

interface TransactionDetailsLayoutProps {
    onClose: () => void;
    transaction?: IotaTransactionBlockResponse;
    digest?: string | null;
}

export function TransactionDetailsLayout({
    onClose,
    transaction: txProp,
    digest,
}: TransactionDetailsLayoutProps) {
    const address = useCurrentAccount()?.address ?? '';
    const recognizedPackagesList = useRecognizedPackages();

    const { data: fetchedTransaction, isError, error } = useGetTransaction(digest ?? '');

    const transaction = txProp ?? fetchedTransaction;

    const summary = useTransactionSummary({
        transaction,
        currentAddress: address,
        recognizedPackagesList,
    });

    const onCopySuccess = useCallback(() => {
        trackElementCopied('transaction-digest');
    }, []);

    if (!txProp && isError) {
        return (
            <InfoBox
                type={InfoBoxType.Error}
                title="Error getting transaction info"
                supportingText={
                    error?.message ?? 'An error occurred when getting the transaction info'
                }
                icon={<Warning />}
                style={InfoBoxStyle.Default}
            />
        );
    }

    if (!transaction || !summary) {
        return (
            <div className="flex h-full w-full justify-center">
                <LoadingIndicator />
            </div>
        );
    }

    return (
        <>
            <Header title="Transaction" onClose={onClose} />
            <DialogLayoutBody>
                <TransactionReceipt
                    txn={transaction}
                    activeAddress={address}
                    summary={summary}
                    renderExplorerLink={ExplorerLink}
                />
            </DialogLayoutBody>
            <DialogLayoutFooter>
                <div className="flex w-full flex-row gap-x-xs">
                    <div className="flex w-full [&_a]:w-full">
                        <ExplorerLink
                            type={ExplorerLinkType.Transaction}
                            transactionID={transaction.digest}
                        >
                            <ViewTxnOnExplorerButton digest={transaction.digest} />
                        </ExplorerLink>
                    </div>
                    <div className="self-center">
                        <OutlinedCopyButton
                            textToCopy={transaction.digest ?? ''}
                            onCopySuccess={onCopySuccess}
                            successMessage="Transaction digest copied to clipboard"
                        />
                    </div>
                </div>
            </DialogLayoutFooter>
        </>
    );
}
