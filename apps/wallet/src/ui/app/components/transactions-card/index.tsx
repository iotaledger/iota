// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    formatDate,
    getBalanceChangeSummary,
    getTransactionAction,
    useFormatCoin,
    useTransactionSummary,
    TransactionIcon,
    checkIfIsTimelockedStaking,
    getTransactionAmountForTimelocked,
    useRecognizedPackages,
    isMigrationTransaction,
} from '@iota/core';
import type { IotaTransactionBlockResponse } from '@iota/iota-sdk/client';
import { Link } from 'react-router-dom';
import {
    Card,
    CardType,
    CardImage,
    ImageType,
    CardBody,
    CardAction,
    CardActionType,
    ImageShape,
} from '@iota/apps-ui-kit';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';

interface TransactionCardProps {
    txn: IotaTransactionBlockResponse;
    address: string;
}

export function TransactionCard({ txn, address }: TransactionCardProps) {
    const executionStatus = txn.effects?.status.status;
    const recognizedPackagesList = useRecognizedPackages();

    const summary = useTransactionSummary({
        transaction: txn,
        currentAddress: address,
        recognizedPackagesList,
    });
    const { isTimelockedStaking, isTimelockedUnstaking } = checkIfIsTimelockedStaking(txn.events);

    // we only show IOTA Transfer amount or the first non-IOTA transfer amount
    // Get the balance changes for the transaction and the amount
    const balanceChanges = getBalanceChangeSummary(txn, recognizedPackagesList);

    const [balance, coinType] = (() => {
        if ((isTimelockedStaking || isTimelockedUnstaking) && txn.events) {
            const balance = getTransactionAmountForTimelocked(
                txn.events,
                isTimelockedStaking,
                isTimelockedUnstaking,
            );
            return [balance, IOTA_TYPE_ARG];
        } else if (isMigrationTransaction(txn.transaction)) {
            const balanceChange = balanceChanges?.[address || '']?.find((change) => {
                return change.coinType === IOTA_TYPE_ARG;
            });
            const balance = balanceChange ? balanceChange.amount : 0;
            return [balance, IOTA_TYPE_ARG];
        } else {
            // Use any non-iota coin type if found, otherwise simply use IOTA
            const nonIotaCoinType = balanceChanges?.[address]
                ?.map((change) => change.coinType)
                .find((coinType) => coinType !== IOTA_TYPE_ARG);
            const coinType = nonIotaCoinType ?? IOTA_TYPE_ARG;
            const balanceChange = balanceChanges?.[address]?.find((change) => {
                return change.coinType === coinType;
            });
            const balance = balanceChange ? balanceChange.amount : 0;
            return [balance, coinType];
        }
    })();

    const [formatAmount, symbol] = useFormatCoin({ balance, coinType });

    const error = txn.effects?.status.error;

    const transactionDate = !txn.timestampMs
        ? '--'
        : formatDate(Number(txn.timestampMs), ['day', 'month', 'year', 'hour', 'minute']);

    return (
        <Link
            data-testid="link-to-txn"
            to={`/receipt?${new URLSearchParams({
                txdigest: txn.digest,
            }).toString()}`}
            className="flex w-full flex-col items-center no-underline"
        >
            <Card type={CardType.Default} isHoverable>
                <CardImage type={ImageType.BgSolid} shape={ImageShape.SquareRounded}>
                    <TransactionIcon
                        txnFailed={executionStatus !== 'success' || !!error}
                        variant={getTransactionAction(txn, address)}
                    />
                </CardImage>
                <CardBody
                    title={error ? 'Transaction Failed' : (summary?.label ?? 'Unknown')}
                    subtitle={transactionDate}
                />
                <CardAction
                    type={CardActionType.SupportingText}
                    title={error ? '--' : `${formatAmount} ${symbol}`}
                />
            </Card>
        </Link>
    );
}
