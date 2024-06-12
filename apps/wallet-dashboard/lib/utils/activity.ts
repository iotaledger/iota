// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Activity, ActivityAction, ActivityState } from '@/lib/interfaces';
import { IotaTransactionBlockResponse } from '@iota/iota.js/client';
import { parseTimestamp } from './time';

const getTransactionActivityState = (tx: IotaTransactionBlockResponse): ActivityState => {
    const executionStatus = tx.effects?.status.status;
    const isTxFailed = !!tx.effects?.status.error;

    if (executionStatus == 'success') {
        return ActivityState.Successful;
    }

    if (isTxFailed) {
        return ActivityState.Failed;
    }

    return ActivityState.Pending;
};

export const getTransactionAction = (
    transaction: IotaTransactionBlockResponse,
    currentAddress: string,
) => {
    const isSender = transaction.transaction?.data.sender === currentAddress;
    return isSender ? ActivityAction.Transaction : ActivityAction.Receive;
};

export const getTransactionActivity = (
    tx: IotaTransactionBlockResponse,
    address: string,
): Activity => {
    return {
        action: getTransactionAction(tx, address),
        state: getTransactionActivityState(tx),
        timestamp: tx.timestampMs ? parseTimestamp(tx.timestampMs) : undefined,
    };
};
