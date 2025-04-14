// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ExplorerLinkHelper, UserApproveContainer } from '_components';
import {
    useActiveAddress,
    useAppDispatch,
    useTransactionData,
    useTransactionDryRun,
    useAccountByAddress,
    useSigner,
} from '_hooks';
import { type TransactionApprovalRequest } from '_src/shared/messaging/messages/payloads/transactions/approvalRequest';
import { respondToTransactionRequest } from '_redux/slices/transaction-requests';
import { ampli } from '_src/shared/analytics/ampli';
import { PageMainLayoutTitle } from '_src/ui/app/shared/page-main-layout/PageMainLayoutTitle';
import {
    useTransactionSummary,
    TransactionSummary,
    GasFees,
    useRecognizedPackages,
} from '@iota/core';
import { Transaction } from '@iota/iota-sdk/transactions';
import { useMemo, useState } from 'react';
import { ConfirmationModal } from '../../../shared/ConfirmationModal';
import { TransactionDetails } from './transaction-details';
import { Warning } from '@iota/apps-ui-icons';
import { InfoBox, InfoBoxType, InfoBoxStyle } from '@iota/apps-ui-kit';

export interface TransactionRequestProps {
    txRequest: TransactionApprovalRequest;
}

// Some applications require *a lot* of transactions to interact with, and this
// eats up our analytics event quota. As a short-term solution so we don't have
// to stop tracking this event entirely, we'll just manually exclude application
// origins with this list
const APP_ORIGINS_TO_EXCLUDE_FROM_ANALYTICS: string[] = [];

export function TransactionRequest({ txRequest }: TransactionRequestProps) {
    const addressForTransaction = txRequest.tx.account;
    const activeAddress = useActiveAddress();
    const { data: accountForTransaction } = useAccountByAddress(addressForTransaction);
    const signer = useSigner(accountForTransaction);
    const dispatch = useAppDispatch();
    const transaction = useMemo(() => {
        const tx = Transaction.from(txRequest.tx.data);
        if (addressForTransaction) {
            tx.setSenderIfNotSet(addressForTransaction);
        }
        return tx;
    }, [txRequest.tx.data, addressForTransaction]);
    const { isPending, isError } = useTransactionData(addressForTransaction, transaction);
    const [isConfirmationVisible, setConfirmationVisible] = useState(false);

    const {
        data,
        isError: isDryRunError,
        isPending: isDryRunLoading,
    } = useTransactionDryRun(addressForTransaction, transaction);
    const recognizedPackagesList = useRecognizedPackages();

    const summary = useTransactionSummary({
        transaction: data,
        currentAddress: addressForTransaction,
        recognizedPackagesList,
    });
    if (!signer) {
        return null;
    }
    return (
        <>
            <UserApproveContainer
                origin={txRequest.origin}
                originFavIcon={txRequest.originFavIcon}
                approveTitle="Approve"
                rejectTitle="Reject"
                onSubmit={async (approved: boolean) => {
                    if (isPending) return;
                    if (approved && isError) {
                        setConfirmationVisible(true);
                        return;
                    }
                    await dispatch(
                        respondToTransactionRequest({
                            approved,
                            txRequestID: txRequest.id,
                            signer,
                        }),
                    );
                    if (!APP_ORIGINS_TO_EXCLUDE_FROM_ANALYTICS.includes(txRequest.origin)) {
                        ampli.respondedToTransactionRequest({
                            applicationUrl: txRequest.origin,
                            approvedTransaction: approved,
                            receivedFailureWarning: false,
                        });
                    }
                }}
                address={addressForTransaction}
                approveLoading={isPending || isConfirmationVisible}
                checkAccountLock
            >
                <PageMainLayoutTitle title="Approve Transaction" />
                <div className="-mr-3 flex flex-col gap-md">
                    <TransactionSummary
                        isDryRun
                        isLoading={isDryRunLoading}
                        isError={isDryRunError}
                        summary={summary}
                        renderExplorerLink={ExplorerLinkHelper}
                    />
                    {(!summary || isDryRunError) && (
                        <InfoBox
                            title="Review the transaction"
                            supportingText="Unexpected issue during the dry run. The transaction may not execute properly."
                            icon={<Warning />}
                            type={InfoBoxType.Default}
                            style={InfoBoxStyle.Elevated}
                        />
                    )}
                    <GasFees
                        sender={addressForTransaction}
                        gasSummary={summary?.gas}
                        isEstimate
                        isError={isError}
                        isPending={isDryRunLoading}
                        activeAddress={activeAddress}
                        renderExplorerLink={ExplorerLinkHelper}
                    />
                    <TransactionDetails sender={addressForTransaction} transaction={transaction} />
                </div>
            </UserApproveContainer>
            <ConfirmationModal
                isOpen={isConfirmationVisible}
                title="Are you sure you want to approve the transaction?"
                hint="This transaction might fail. You will still be charged a gas fee for this transaction."
                confirmText="Approve"
                cancelText="Reject"
                onResponse={async (isConfirmed) => {
                    await dispatch(
                        respondToTransactionRequest({
                            approved: isConfirmed,
                            txRequestID: txRequest.id,
                            signer,
                        }),
                    );
                    ampli.respondedToTransactionRequest({
                        applicationUrl: txRequest.origin,
                        approvedTransaction: isConfirmed,
                        receivedFailureWarning: true,
                    });
                    setConfirmationVisible(false);
                }}
            />
        </>
    );
}
