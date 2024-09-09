// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useRecognizedPackages } from '_src/ui/app/hooks/useRecognizedPackages';
import {
    useTransactionSummary,
    STAKING_REQUEST_EVENT,
    UNSTAKING_REQUEST_EVENT,
    formatDate,
} from '@iota/core';
import { type IotaTransactionBlockResponse } from '@iota/iota-sdk/client';

import { TransactionSummary } from '../../shared/transaction-summary';
import { StakeTxn } from './StakeTxn';
import { UnStakeTxn } from './UnstakeTxn';
import { Button, ButtonType, Card, CardBody, CardType } from '@iota/apps-ui-kit';
import { useNavigate } from 'react-router-dom';
import { CheckmarkFilled } from '@iota/ui-icons';
import cl from 'clsx';

interface TransactionStatusProps {
    success: boolean;
    timestamp?: string;
}

function TransactionStatus({ success, timestamp }: TransactionStatusProps) {
    const txnDate = formatDate(Number(timestamp) ?? '');
    return (
        <Card type={CardType.Filled}>
            <CheckmarkFilled
                className={cl('h-5 w-5', success ? 'text-primary-30' : 'text-neutral-10')}
            />
            <CardBody
                title={success ? 'Successfully sent' : 'Transaction Failed'}
                subtitle={timestamp ? txnDate : ''}
            />
        </Card>
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
                {stakedTxn ? <StakeTxn event={stakedTxn} gasSummary={summary?.gas} /> : null}
                {unstakeTxn ? <UnStakeTxn event={unstakeTxn} gasSummary={summary?.gas} /> : null}
                <Button type={ButtonType.Primary} text="Finish" onClick={handleCancel} fullWidth />
            </div>
        );

    return (
        <div className="h-full w-full overflow-y-auto overflow-x-hidden">
            <TransactionStatus
                success={summary.status === 'success'}
                timestamp={txn.timestampMs ?? undefined}
            />
            <TransactionSummary showGasSummary summary={summary} />
        </div>
    );
}
