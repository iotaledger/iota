// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ExplorerLink } from '@/components';
import { Header, LoadingIndicator } from '@iota/apps-ui-kit';
import {
    useTransactionSummary,
    ViewTxnOnExplorerButton,
    ExplorerLinkType,
    TransactionReceipt,
    useRecognizedPackages,
    ExtendedTransaction,
} from '@iota/core';
import { useCurrentAccount } from '@iota/dapp-kit';
import { DialogLayoutBody, DialogLayoutFooter } from '../layout';

interface TransactionDialogDetailsProps {
    transaction: ExtendedTransaction;
    onClose: () => void;
}
export function TransactionDetailsLayout({ transaction, onClose }: TransactionDialogDetailsProps) {
    const address = useCurrentAccount()?.address ?? '';

    const recognizedPackagesList = useRecognizedPackages();
    const summary = useTransactionSummary({
        transaction: transaction.raw,
        currentAddress: address,
        recognizedPackagesList,
    });

    if (!summary) return <LoadingIndicator />;

    return (
        <>
            <Header title="Transaction" onClose={onClose} />
            <DialogLayoutBody>
                <TransactionReceipt
                    txn={transaction.raw}
                    activeAddress={address}
                    summary={summary}
                    renderExplorerLink={ExplorerLink}
                />
            </DialogLayoutBody>
            <DialogLayoutFooter>
                <ExplorerLink
                    type={ExplorerLinkType.Transaction}
                    transactionID={transaction.raw.digest}
                >
                    <ViewTxnOnExplorerButton digest={transaction.raw.digest} />
                </ExplorerLink>
            </DialogLayoutFooter>
        </>
    );
}
