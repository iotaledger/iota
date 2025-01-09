// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { useState } from 'react';
import { TransactionIcon } from './TransactionIcon';
import formatTimestamp from '@/lib/utils/time';
import { ExtendedTransaction, TransactionState } from '@/lib/interfaces';
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
import { useFormatCoin, getLabel, useTransactionSummary } from '@iota/core';
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
    const [formatAmount, symbol] = useFormatCoin(
        Math.abs(Number(address ? transactionSummary?.balanceChanges?.[address]?.[0]?.amount : 0)),
        IOTA_TYPE_ARG,
    );

    function openDetailsDialog() {
        setOpen(true);
    }

    const transactionDate = transaction?.timestamp && formatTimestamp(transaction.timestamp);

    return (
        <>
            <Card type={CardType.Default} isHoverable onClick={openDetailsDialog}>
                <CardImage type={ImageType.BgSolid} shape={ImageShape.SquareRounded}>
                    <TransactionIcon
                        txnFailed={transaction.state === TransactionState.Failed}
                        variant={getLabel(transaction?.raw, address)}
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
