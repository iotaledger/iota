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
    useRecognizedPackages,
} from '@iota/core';
import { useCurrentAccount, useIotaClientContext } from '@iota/dapp-kit';
import { DialogLayoutBody, DialogLayoutFooter } from '../layout';
import { Validator } from '../Staking/views/Validator';
import { Network } from '@iota/iota-sdk/client';

interface TransactionDialogDetailsProps {
    transaction: ExtendedTransaction;
    onClose: () => void;
}
export function TransactionDetailsLayout({
    transaction,
    onClose
}: TransactionDialogDetailsProps) {
    const address = useCurrentAccount()?.address ?? '';

    const { network } = useIotaClientContext();
    const recognizedPackagesList = useRecognizedPackages(network as Network);
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
        </>
    );
}
