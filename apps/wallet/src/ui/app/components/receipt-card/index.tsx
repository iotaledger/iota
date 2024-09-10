// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useRecognizedPackages } from '_src/ui/app/hooks/useRecognizedPackages';
import { useTransactionSummary, STAKING_REQUEST_EVENT, UNSTAKING_REQUEST_EVENT } from '@iota/core';
import { type IotaTransactionBlockResponse } from '@iota/iota-sdk/client';

import { DateCard } from '../../shared/date-card';
import { TransactionSummary } from '../../shared/transaction-summary';
import { StakeTxn } from './StakeTxn';
import { StatusIcon } from './StatusIcon';
import { UnStakeTxn } from './UnstakeTxn';
import { ExplorerLinkCard } from '../../shared/transaction-summary/cards/ExplorerLink';

interface TransactionStatusProps {
    success: boolean;
    timestamp?: string;
}

function TransactionStatus({ success, timestamp }: TransactionStatusProps) {
    return (
        <div className="mb-4 flex flex-col items-center justify-center gap-3">
            <StatusIcon status={success} />
            <span data-testid="transaction-status" className="sr-only">
                {success ? 'Transaction Success' : 'Transaction Failed'}
            </span>
            {timestamp && <DateCard timestamp={Number(timestamp)} size="md" />}
        </div>
    );
}

interface ReceiptCardProps {
    txn: IotaTransactionBlockResponse;
    activeAddress: string;
}

export function ReceiptCard({ txn, activeAddress }: ReceiptCardProps) {
    const { events } = txn;
    const recognizedPackagesList = useRecognizedPackages();
    const summary = useTransactionSummary({
        transaction: txn,
        currentAddress: activeAddress,
        recognizedPackagesList,
    });

    if (!summary) return null;

    const stakedTxn = events?.find(({ type }) => type === STAKING_REQUEST_EVENT);

    const unstakeTxn = events?.find(({ type }) => type === UNSTAKING_REQUEST_EVENT);

    // todo: re-using the existing staking cards for now
    if (stakedTxn || unstakeTxn)
        return (
            <div className="flex h-full flex-col justify-between">
                {stakedTxn ? <StakeTxn event={stakedTxn} gasSummary={summary?.gas} /> : null}
                {unstakeTxn ? <UnStakeTxn event={unstakeTxn} gasSummary={summary?.gas} /> : null}
                <ExplorerLinkCard
                    digest={summary?.digest}
                    timestamp={summary?.timestamp ?? undefined}
                />
            </div>
        );

    return (
        <div className="relative block h-full w-full">
            <TransactionStatus
                success={summary.status === 'success'}
                timestamp={txn.timestampMs ?? undefined}
            />
            <TransactionSummary showGasSummary summary={summary} />
        </div>
    );
}
