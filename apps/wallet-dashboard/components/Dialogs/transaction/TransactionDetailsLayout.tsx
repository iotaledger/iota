// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import React from 'react';
import { ExplorerLink } from '@/components';
import { ExtendedTransaction } from '@/lib/interfaces';
import { Header, LoadingIndicator } from '@iota/apps-ui-kit';
import {
    useTransactionSummary,
    ViewTxnOnExplorerButton,
    ExplorerLinkType,
    TransactionReceipt,
} from '@iota/core';
import { useCurrentAccount } from '@iota/dapp-kit';
import { DialogLayout, DialogLayoutBody, DialogLayoutFooter } from '../layout';
import { Validator } from '../Staking/views/Validator';

interface TransactionDialogDetailsProps {
    transaction: ExtendedTransaction;
    onClose: () => void;
    withDialogContent?: boolean;
}
export function TransactionDetailsLayout({
    transaction,
    onClose,
    withDialogContent,
}: TransactionDialogDetailsProps) {
    const address = useCurrentAccount()?.address ?? '';

    const summary = useTransactionSummary({
        transaction: transaction.raw,
        currentAddress: address,
        recognizedPackagesList: [],
    });

    if (!summary) return <LoadingIndicator />;

    return (
        <DialogLayout withDialogContent={withDialogContent}>
            <Header title="Transaction" onClose={onClose} />
            <DialogLayoutBody>
                <TransactionReceipt
                    txn={transaction.raw}
                    activeAddress={address}
                    summary={summary}
                    renderExplorerLink={ExplorerLink}
                    renderValidatorLogo={Validator}
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
        </DialogLayout>
    );
}