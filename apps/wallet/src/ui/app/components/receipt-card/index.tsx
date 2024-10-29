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
import {
    Button,
    ButtonType,
    InfoBox,
    InfoBoxStyle,
    InfoBoxType,
    LoadingIndicator,
} from '@iota/apps-ui-kit';
import { ArrowTopRight, CheckmarkFilled } from '@iota/ui-icons';
import { GasFees } from '../../pages/approval-request/transaction-request/GasFees';
import ExplorerLink, { ExplorerLinkType } from '../explorer-link';

interface TransactionStatusProps {
    success: boolean;
    timestamp?: string;
}

function TransactionStatus({ success, timestamp }: TransactionStatusProps) {
    const txnDate = timestamp ? formatDate(Number(timestamp)) : '';
    return (
        <InfoBox
            type={success ? InfoBoxType.Default : InfoBoxType.Error}
            style={InfoBoxStyle.Elevated}
            title={success ? 'Successfully sent' : 'Transaction Failed'}
            supportingText={timestamp ? txnDate : ''}
            icon={<CheckmarkFilled />}
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

    const { digest } = summary;
    const stakedTxn = events?.find(({ type }) => type === STAKING_REQUEST_EVENT);
    const unstakeTxn = events?.find(({ type }) => type === UNSTAKING_REQUEST_EVENT);

    return (
        <div className="flex h-full w-full flex-col justify-between">
            <div className="flex flex-col gap-md overflow-y-auto overflow-x-hidden">
                <TransactionStatus
                    success={summary.status === 'success'}
                    timestamp={txn.timestampMs ?? undefined}
                />
                {stakedTxn || unstakeTxn ? (
                    <>
                        {stakedTxn ? (
                            <StakeTxn event={stakedTxn} gasSummary={summary?.gas} />
                        ) : null}
                        {unstakeTxn ? (
                            <UnStakeTxn event={unstakeTxn} gasSummary={summary?.gas} />
                        ) : null}
                    </>
                ) : (
                    <>
                        <TransactionSummary summary={summary} />
                        {isSender && <GasFees gasSummary={summary?.gas} />}
                    </>
                )}
            </div>
            <div className="pt-sm">
                <ExplorerLink transactionID={digest ?? ''} type={ExplorerLinkType.Transaction}>
                    <Button
                        type={ButtonType.Outlined}
                        text="View on Explorer"
                        fullWidth
                        icon={
                            digest ? (
                                <ArrowTopRight />
                            ) : (
                                <LoadingIndicator data-testid="loading-indicator" />
                            )
                        }
                        iconAfterText
                        disabled={!digest}
                    />
                </ExplorerLink>
            </div>
        </div>
    );
}
