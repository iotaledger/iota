// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import React, { useState } from 'react';
import TransactionIcon from './TransactionIcon';
import formatTimestamp from '@/lib/utils/time';
import { ExplorerLink } from '@/components';
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
    Header,
    LoadingIndicator,
} from '@iota/apps-ui-kit';
import {
    useFormatCoin,
    getLabel,
    useTransactionSummary,
    ViewTxnOnExplorerButton,
    ExplorerLinkType,
    TransactionReceipt,
} from '@iota/core';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { useCurrentAccount } from '@iota/dapp-kit';
import { DialogLayout, DialogLayoutBody, DialogLayoutFooter } from '../Dialogs/layout';
import { Validator } from '../Dialogs/Staking/views/Validator';

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
            <ActivityDetailsDialog transaction={transaction} open={open} setOpen={setOpen} />
        </>
    );
}

interface ActivityDetailsDialogProps {
    transaction: ExtendedTransaction;
    open: boolean;
    setOpen: (open: boolean) => void;
}
function ActivityDetailsDialog({
    transaction,
    open,
    setOpen,
}: ActivityDetailsDialogProps): React.JSX.Element {
    const address = useCurrentAccount()?.address ?? '';

    const summary = useTransactionSummary({
        transaction: transaction.raw,
        currentAddress: address,
        recognizedPackagesList: [],
    });

    if (!summary) return <LoadingIndicator />;

    return (
        <Dialog open={open} onOpenChange={setOpen}>
            <DialogLayout>
                <Header title="Transaction" onClose={() => setOpen(false)} />
                <DialogLayoutBody>
                    <TransactionReceipt
                        txn={transaction.raw}
                        activeAddress={address}
                        summary={summary}
                        renderExplorerLink={ExplorerLink}
                        renderValidatorLogo={Validator}
                    />
                </DialogLayoutBody>
                <DialogLayoutFooter>
                    <ExplorerLink
                        type={ExplorerLinkType.Transaction}
                        transactionID={transaction.raw.digest}
                    >
                        <ViewTxnOnExplorerButton digest={transaction.raw.digest} />
                    </ExplorerLink>
                </DialogLayoutFooter>
            </DialogLayout>
        </Dialog>
    );
}
