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
import { Card, CardBody, CardType, InfoBox, InfoBoxStyle, InfoBoxType } from '@iota/apps-ui-kit';
import { CheckmarkFilled } from '@iota/ui-icons';
import cl from 'clsx';
import { ExplorerLinkCard } from '../../shared/transaction-summary/cards/ExplorerLink';
import { GasFees } from '../../pages/approval-request/transaction-request/GasFees';

interface TransactionStatusProps {
    success: boolean;
    timestamp?: string;
}

function TransactionStatus({ success, timestamp }: TransactionStatusProps) {
    const txnDate = timestamp ? formatDate(Number(timestamp)) : '';
    return (
        <InfoBox
            type={success ? InfoBoxType.Default : InfoBoxType.Warning}
            style={InfoBoxStyle.Elevated}
            title={success ? 'Successfully sent' : 'Transaction Failed'}
            supportingText={timestamp ? txnDate : ''}
            icon={
                <CheckmarkFilled
                    className={cl('h-5 w-5', success ? 'text-primary-30' : 'text-neutral-10')}
                />
            }
        ></InfoBox>
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
    const isSender = txn.transaction?.data.sender === activeAddress;

    if (!summary) return null;

    const stakedTxn = events?.find(({ type }) => type === STAKING_REQUEST_EVENT);

    const unstakeTxn = events?.find(({ type }) => type === UNSTAKING_REQUEST_EVENT);

    const renderExplorerLinkCard = () => (
        <ExplorerLinkCard digest={summary?.digest} timestamp={summary?.timestamp ?? undefined} />
    );

    // todo: re-using the existing staking cards for now
    if (stakedTxn || unstakeTxn)
        return (
            <div className="flex h-full w-full flex-col justify-between">
                {stakedTxn ? <StakeTxn event={stakedTxn} gasSummary={summary?.gas} /> : null}
                {unstakeTxn ? <UnStakeTxn event={unstakeTxn} gasSummary={summary?.gas} /> : null}
                {renderExplorerLinkCard()}
            </div>
        );

    return (
        <div className="flex h-full w-full flex-col">
            <div className="-mr-3 flex flex-col gap-md overflow-y-auto overflow-x-hidden">
                <TransactionStatus
                    success={summary.status === 'success'}
                    timestamp={txn.timestampMs ?? undefined}
                />
                <TransactionSummary summary={summary} />
                {isSender && <GasFees gasSummary={summary?.gas} />}
            </div>
            <div className="pt-sm">{renderExplorerLinkCard()}</div>
        </div>
    );
}
