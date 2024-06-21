// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { PropsWithChildren } from 'react';
import { ExtendedTransaction } from '@/lib/interfaces';
import {
    formatDate,
    useExtendedTransactionSummary,
    STAKING_REQUEST_EVENT,
    UNSTAKING_REQUEST_EVENT,
} from '@iota/core';
import {
    StakeTransactionCard,
    TransactionSummary,
    UnstakeTransactionCard,
} from '@/components/Transaction';

interface TransactionDetailsPopupProps {
    transaction: ExtendedTransaction;
    onClose: () => void;
}

function LabeledValue({ label, children }: PropsWithChildren<{ label: string }>): JSX.Element {
    return (
        <div className="flex flex-row gap-2">
            <h3 className="text-md">
                <span className="font-semibold">{label}</span>:
            </h3>
            <p className="capitalize">{children}</p>
        </div>
    );
}

export default function TransactionDetailsPopup({
    transaction,
    onClose,
}: TransactionDetailsPopupProps): JSX.Element {
    const { raw: rawTransaction } = transaction;
    const { events } = rawTransaction;

    const transactionSummary = useExtendedTransactionSummary(rawTransaction.digest);

    const txDate = !Number.isNaN(rawTransaction.timestampMs)
        ? formatDate(Number(rawTransaction.timestampMs), ['month', 'day', 'hour', 'minute'])
        : undefined;

    const stakedTxn = events?.find(({ type }) => type === STAKING_REQUEST_EVENT);
    const unstakeTxn = events?.find(({ type }) => type === UNSTAKING_REQUEST_EVENT);

    return (
        <div className="flex w-full min-w-[30vw] flex-col gap-6">
            <div className="flex w-full flex-col">
                <h2 className="mx-auto font-semibold">Transaction Details</h2>
            </div>

            <div className="flex flex-col space-y-1">
                <LabeledValue label="Status">{transaction.state}</LabeledValue>
                <LabeledValue label="Action">{transaction.action}</LabeledValue>
                <LabeledValue label="Timestamp">{txDate}</LabeledValue>
            </div>

            <div className="w-full py-2">
                <div className="mb-2 block text-center font-bold">Transaction Summary</div>
                <TransactionSummary summary={transactionSummary} showGasSummary />
            </div>

            {(stakedTxn || unstakeTxn) && (
                <div className="flex flex-col space-y-2 rounded-lg">
                    {stakedTxn && <StakeTransactionCard event={stakedTxn} />}
                    {unstakeTxn && <UnstakeTransactionCard event={unstakeTxn} />}
                </div>
            )}
        </div>
    );
}
