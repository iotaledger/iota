// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useRecognizedPackages } from '_src/ui/app/hooks/useRecognizedPackages';
import { useTransactionSummary, STAKING_REQUEST_EVENT, UNSTAKING_REQUEST_EVENT } from '@iota/core';
import { type IotaTransactionBlockResponse } from '@iota/iota-sdk/client';

import { DateCard } from '../../shared/date-card';
import { TransactionSummary } from '../../shared/transaction-summary';
import { ExplorerLinkCard } from '../../shared/transaction-summary/cards/ExplorerLink';
import { GasSummary } from '../../shared/transaction-summary/cards/GasSummary';
import { StakeTxnCard } from './StakeTxnCard';
import { StatusIcon } from './StatusIcon';
import { UnStakeTxnCard } from './UnstakeTxnCard';

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
            <div className="relative block h-full w-full">
                <section className="bg-iota/10 -mx-5 min-h-full">
                    <div className="px-5 py-10">
                        <div className="flex flex-col gap-4">
                            {stakedTxn ? <StakeTxnCard event={stakedTxn} /> : null}
                            {unstakeTxn ? <UnStakeTxnCard event={unstakeTxn} /> : null}
                            <GasSummary gasSummary={summary?.gas} />
                            <ExplorerLinkCard
                                digest={summary?.digest}
                                timestamp={summary?.timestamp ?? undefined}
                            />
                        </div>
                    </div>
                </section>
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
