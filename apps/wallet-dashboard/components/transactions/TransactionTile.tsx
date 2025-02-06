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
    ExtendedTransaction,
    TransactionState,
    TransactionIcon,
    checkIfIsTimelockedStaking,
    getTransactionAmountForTimelocked,
    formatDate,
} from '@iota/core';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { useCurrentAccount } from '@iota/dapp-kit';
import { TransactionDetailsLayout } from '../dialogs/transaction/TransactionDetailsLayout';
import { DialogLayout } from '../dialogs/layout';

interface TransactionTileProps {
    transaction: ExtendedTransaction;
}

export function TransactionTile({ transaction }: TransactionTileProps): JSX.Element {
    const account = useCurrentAccount();
    const address = account?.address;
    const [open, setOpen] = useState(false);

    const transactionSummary = useTransactionSummary({
        transaction: transaction.raw,
        currentAddress: account?.address,
        recognizedPackagesList: [],
    });

    const { isTimelockedStaking, isTimelockedUnstaking } = checkIfIsTimelockedStaking(
        transaction.raw?.events,
    );

    const balanceChanges = transactionSummary?.balanceChanges;

    function getAmount(tx: ExtendedTransaction) {
        if ((isTimelockedStaking || isTimelockedUnstaking) && tx.raw.events) {
            return getTransactionAmountForTimelocked(
                tx.raw.events,
                isTimelockedStaking,
                isTimelockedUnstaking,
            );
        } else {
            return address && balanceChanges?.[address]?.[0]?.amount
                ? Math.abs(Number(balanceChanges?.[address]?.[0]?.amount))
                : 0;
        }
    }

    const transactionAmount = getAmount(transaction);
    const [formatAmount, symbol] = useFormatCoin(transactionAmount, IOTA_TYPE_ARG);

    function openDetailsDialog() {
        setOpen(true);
    }

    const transactionDate =
        transaction?.timestamp &&
        formatDate(Number(transaction?.timestamp), ['day', 'month', 'year', 'hour', 'minute']);

    return (
        <>
            <Card type={CardType.Default} isHoverable onClick={openDetailsDialog}>
                <CardImage type={ImageType.BgSolid} shape={ImageShape.SquareRounded}>
                    <TransactionIcon
                        txnFailed={transaction.state === TransactionState.Failed}
                        variant={getTransactionAction(transaction?.raw, address)}
                    />
                </CardImage>
                <CardBody
                    title={
                        transaction.state === TransactionState.Failed
                            ? 'Transaction Failed'
                            : (transaction.action ?? 'Unknown')
                    }
                    subtitle={transactionDate}
                />
                <CardAction
                    type={CardActionType.SupportingText}
                    title={
                        transaction.state === TransactionState.Failed
                            ? '--'
                            : `${formatAmount} ${symbol}`
                    }
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
