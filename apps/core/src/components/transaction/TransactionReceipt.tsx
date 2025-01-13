// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { InfoBox, InfoBoxStyle, InfoBoxType } from '@iota/apps-ui-kit';
import type { useTransactionSummary } from '../../hooks';
import { CheckmarkFilled } from '@iota/ui-icons';
import { IotaTransactionBlockResponse } from '@iota/iota-sdk/client';
import { STAKING_REQUEST_EVENT, UNSTAKING_REQUEST_EVENT } from '../../constants';
import { StakeTransactionDetails } from './details';
import { UnstakeTransactionInfo } from './info';
import { TransactionSummary } from './summary';
import { RenderExplorerLink, RenderValidatorLogo } from '../../types';
import { GasFees } from '../gas';
import { formatDate } from '../../utils';

interface TransactionReceiptProps {
    txn: IotaTransactionBlockResponse;
    activeAddress: string | null;
    summary: Exclude<ReturnType<typeof useTransactionSummary>, null>;
    renderValidatorLogo: RenderValidatorLogo;
    renderExplorerLink: RenderExplorerLink;
}

export function TransactionReceipt({
    txn,
    activeAddress,
    summary,
    renderValidatorLogo,
    renderExplorerLink,
}: TransactionReceiptProps) {
    const { events } = txn;

    const isSender = txn.transaction?.data.sender === activeAddress;

    const stakeTypeTransaction = events?.find(({ type }) => type === STAKING_REQUEST_EVENT);
    const unstakeTypeTransaction = events?.find(({ type }) => type === UNSTAKING_REQUEST_EVENT);

    return (
        <div className="flex flex-col gap-md overflow-y-auto overflow-x-hidden">
            <TransactionStatus
                success={summary.status === 'success'}
                timestamp={txn.timestampMs ?? undefined}
                isIncoming={!isSender}
            />
            {stakeTypeTransaction || unstakeTypeTransaction ? (
                <>
                    {stakeTypeTransaction ? (
                        <StakeTransactionDetails
                            activeAddress={activeAddress}
                            event={stakeTypeTransaction}
                            gasSummary={summary?.gas}
                            renderValidatorLogo={renderValidatorLogo}
                            renderExplorerLink={renderExplorerLink}
                        />
                    ) : null}

                    {unstakeTypeTransaction ? (
                        <UnstakeTransactionInfo
                            activeAddress={activeAddress}
                            event={unstakeTypeTransaction}
                            gasSummary={summary?.gas}
                            renderExplorerLink={renderExplorerLink}
                            renderValidatorLogo={renderValidatorLogo}
                        />
                    ) : null}
                </>
            ) : (
                <>
                    <TransactionSummary summary={summary} renderExplorerLink={renderExplorerLink} />
                    {isSender && (
                        <GasFees
                            gasSummary={summary?.gas}
                            renderExplorerLink={renderExplorerLink}
                            activeAddress={activeAddress}
                        />
                    )}
                </>
            )}
        </div>
    );
}

interface TransactionStatusProps {
    success: boolean;
    timestamp?: string;
    isIncoming?: boolean;
}

function TransactionStatus({ success, timestamp, isIncoming }: TransactionStatusProps) {
    const txnDate = timestamp
        ? formatDate(Number(timestamp), ['day', 'month', 'year', 'hour', 'minute'])
        : '';
    const successMessage = isIncoming ? 'Successfully received' : 'Successfully sent';
    return (
        <InfoBox
            type={success ? InfoBoxType.Default : InfoBoxType.Error}
            style={InfoBoxStyle.Elevated}
            title={success ? successMessage : 'Transaction Failed'}
            supportingText={timestamp ? txnDate : ''}
            icon={<CheckmarkFilled />}
        />
    );
}
