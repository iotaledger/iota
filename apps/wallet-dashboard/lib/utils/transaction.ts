// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ExtendedTransaction, TransactionAction, TransactionState } from '@/lib/interfaces';
import { IotaTransactionBlockResponse } from '@iota/iota-sdk/client';
import { parseTimestamp } from './time';

const getTransactionTransactionState = (tx: IotaTransactionBlockResponse): TransactionState => {
    const executionStatus = tx.effects?.status.status;
    const isTxFailed = !!tx.effects?.status.error;

    if (executionStatus == 'success') {
        return TransactionState.Successful;
    }

    if (isTxFailed) {
        return TransactionState.Failed;
    }

    return TransactionState.Pending;
};

export const getTransactionAction = (
    transaction: IotaTransactionBlockResponse,
    currentAddress: string,
) => {
    const isSender = transaction.transaction?.data.sender === currentAddress;
    return isSender ? TransactionAction.Send : TransactionAction.Receive;
};

export const getExtendedTransaction = (
    tx: IotaTransactionBlockResponse,
    address: string,
): ExtendedTransaction => {
    return {
        action: getTransactionAction(tx, address),
        state: getTransactionTransactionState(tx),
        timestamp: tx.timestampMs ? parseTimestamp(tx.timestampMs) : undefined,
        raw: tx,
    };
};
