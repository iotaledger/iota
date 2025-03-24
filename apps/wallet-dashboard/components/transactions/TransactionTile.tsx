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
    isMigrationTransaction,
} from '@iota/core';
import { useCurrentAccount } from '@iota/dapp-kit';
import { TransactionDetailsLayout } from '../dialogs/transaction/TransactionDetailsLayout';
import { DialogLayout } from '../dialogs/layout';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';

interface TransactionTileProps {
    transaction: ExtendedTransaction;
}

export function TransactionTile({ transaction }: TransactionTileProps): JSX.Element {
    const account = useCurrentAccount();
    const address = account?.address;
    const [open, setOpen] = useState(false);

    const transactionSummary = useTransactionSummary({
        transaction: transaction.raw,
        currentAddress: address,
        recognizedPackagesList: [],
    });

    const { isTimelockedStaking, isTimelockedUnstaking } = checkIfIsTimelockedStaking(
        transaction.raw?.events,
    );

    const balanceChanges = transactionSummary?.balanceChanges;

    const [balance, coinType] = (() => {
        if ((isTimelockedStaking || isTimelockedUnstaking) && transaction.raw.events) {
            const balance = getTransactionAmountForTimelocked(
                transaction.raw.events,
                isTimelockedStaking,
                isTimelockedUnstaking,
            );
            return [balance, IOTA_TYPE_ARG];
        } else if (isMigrationTransaction(transaction.raw.transaction)) {
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
