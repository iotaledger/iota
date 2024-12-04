// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import React from 'react';
import TransactionIcon from './TransactionIcon';
import formatTimestamp from '@/lib/utils/time';
import { usePopups } from '@/hooks';
import { TransactionDetailsPopup } from '@/components';
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
} from '@iota/apps-ui-kit';
import { useFormatCoin, useExtendedTransactionSummary, getLabel } from '@iota/core';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { useCurrentAccount } from '@iota/dapp-kit';

interface TransactionTileProps {
    transaction: ExtendedTransaction;
}

function TransactionTile({ transaction }: TransactionTileProps): JSX.Element {
    const account = useCurrentAccount();
    const address = account?.address;
    const { openPopup, closePopup } = usePopups();

    const transactionSummary = useExtendedTransactionSummary(transaction.raw.digest);
    const [formatAmount, symbol] = useFormatCoin(
        Math.abs(Number(address ? transactionSummary?.balanceChanges?.[address]?.[0]?.amount : 0)),
        IOTA_TYPE_ARG,
    );

    const handleDetailsClick = () => {
        openPopup(<TransactionDetailsPopup transaction={transaction} onClose={closePopup} />);
    };

    const transactionDate = transaction?.timestamp && formatTimestamp(transaction.timestamp);
    return (
        <Card type={CardType.Default} isHoverable onClick={handleDetailsClick}>
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
    );
}

export default TransactionTile;
