// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useRecognizedPackages } from '_src/ui/app/hooks/useRecognizedPackages';
import { useTransactionSummary, STAKING_REQUEST_EVENT, UNSTAKING_REQUEST_EVENT } from '@iota/core';
import { type IotaTransactionBlockResponse } from '@iota/iota-sdk/client';

import { DateCard } from '../../shared/date-card';
import { TransactionSummary } from '../../shared/transaction-summary';
import { StakeTxnCard } from './StakeTxnCard';
import { StatusIcon } from './StatusIcon';
import { UnStakeTxnCard } from './UnstakeTxnCard';
import { Button, ButtonType } from '@iota/apps-ui-kit';
import { useNavigate } from 'react-router-dom';

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
    const navigate = useNavigate();

    if (!summary) return null;

    function handleCancel() {
        navigate('/');
    }

    const stakedTxn = events?.find(({ type }) => type === STAKING_REQUEST_EVENT);

    const unstakeTxn = events?.find(({ type }) => type === UNSTAKING_REQUEST_EVENT);

    // todo: re-using the existing staking cards for now
    if (stakedTxn || unstakeTxn)
        return (
            <div className="flex h-full flex-col justify-between">
                {stakedTxn ? <StakeTxnCard event={stakedTxn} gasSummary={summary?.gas} /> : null}
                {unstakeTxn ? (
                    <UnStakeTxnCard event={unstakeTxn} gasSummary={summary?.gas} />
                ) : null}
                <Button type={ButtonType.Primary} text="Finish" onClick={handleCancel} fullWidth />
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
