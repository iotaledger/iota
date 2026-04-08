// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { useState } from 'react';
import {
    Card,
    CardType,
    CardImage,
    ImageType,
    ImageShape,
    CardBody,
    CardAction,
    CardActionType,
    Dialog,
} from '@iota/apps-ui-kit';
import {
    useFormatCoin,
    getTransactionAction,
    useTransactionSummary,
    TransactionIcon,
    checkIfIsTimelockedStaking,
    getTransactionAmountForTimelocked,
    formatDate,
    isMigrationTransaction,
} from '@iota/core';
import { useCurrentAccount } from '@iota/dapp-kit';
import { TransactionDetailsLayout } from '../dialogs/transaction/TransactionDetailsLayout';
import { DialogLayout } from '../dialogs/layout';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { IotaTransactionBlockResponse } from '@iota/iota-sdk/client';

interface TransactionTileProps {
    transaction: IotaTransactionBlockResponse;
}

export function TransactionTile({ transaction }: TransactionTileProps): JSX.Element {
    const account = useCurrentAccount();
    const address = account?.address;
    const [open, setOpen] = useState(false);

    const transactionSummary = useTransactionSummary({
        transaction,
        currentAddress: address,
        recognizedPackagesList: [],
    });

    const { isTimelockedStaking, isTimelockedUnstaking } = checkIfIsTimelockedStaking(
        transaction?.events,
    );

    const balanceChanges = transactionSummary?.balanceChanges;
    const txnFailed = transactionSummary?.status !== 'success';
    const label = transactionSummary?.label;

    const [balance, coinType] = (() => {
        if ((isTimelockedStaking || isTimelockedUnstaking) && transaction.events) {
            const balance = getTransactionAmountForTimelocked(
                transaction.events,
                isTimelockedStaking,
                isTimelockedUnstaking,
            );
            return [balance, IOTA_TYPE_ARG];
        } else if (isMigrationTransaction(transaction.transaction)) {
            const balanceChange = balanceChanges?.[address || '']?.find((change) => {
                return change.coinType === IOTA_TYPE_ARG;
            });
            const balance = balanceChange ? balanceChange.amount : 0;
            return [balance, IOTA_TYPE_ARG];
        } else {
            // Use any non-iota coin type if found, otherwise simply use IOTA
            const nonIotaCoinType = balanceChanges?.[address || '']
                ?.map((change) => change.coinType)
                .find((coinType) => coinType !== IOTA_TYPE_ARG);
            const coinType = nonIotaCoinType ?? IOTA_TYPE_ARG;
            const balanceChange = balanceChanges?.[address || '']?.find((change) => {
                return change.coinType === coinType;
            });
            const balance = balanceChange ? balanceChange.amount : 0;
            return [balance, coinType];
        }
    })();

    const [formatAmount, symbol] = useFormatCoin({ balance, coinType });

    function openDetailsDialog() {
        setOpen(true);
    }

    const transactionDate =
        transactionSummary?.timestamp &&
        formatDate(Number(transactionSummary.timestamp), [
            'day',
            'month',
            'year',
            'hour',
            'minute',
        ]);

    return (
        <>
            <Card
                testId="transaction-tile"
                type={CardType.Default}
                isHoverable
                onClick={openDetailsDialog}
                aria-label={`View ${label ?? 'transaction'} details`}
            >
                <CardImage type={ImageType.BgSolid} shape={ImageShape.SquareRounded}>
                    <TransactionIcon
                        variant={getTransactionAction(transaction, address)}
                        txnFailed={txnFailed}
                    />
                </CardImage>
                <CardBody
                    title={txnFailed ? `Failed - ${label ?? 'Unknown'}` : (label ?? 'Unknown')}
                    subtitle={transactionDate}
                />
                <CardAction
                    type={CardActionType.SupportingText}
                    title={txnFailed ? '--' : `${formatAmount} ${symbol}`}
                />
            </Card>
            <Dialog open={open} onOpenChange={setOpen}>
                <DialogLayout>
                    <TransactionDetailsLayout
                        transaction={transaction}
                        onClose={() => setOpen(false)}
                    />
                </DialogLayout>
            </Dialog>
        </>
    );
}
