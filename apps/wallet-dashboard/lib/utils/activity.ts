// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Activity, ActivityAction, ActivityState } from '@/lib/interfaces';
import { ExecutionStatus, IotaTransactionBlockResponse } from '@iota/iota.js/client';
import { parseTimestamp } from './time';

const getTransactionActivityState = (tx: IotaTransactionBlockResponse): ActivityState => {
    const executionStatus = tx.effects?.status.status;
    const isTxFailed = !!tx.effects?.status.error;
    const map: {
        [key in ExecutionStatus['status']]: ActivityState;
    } = {
        success: ActivityState.Successful,
        failure: ActivityState.Failed,
    };

    if (executionStatus !== 'success' && isTxFailed) {
        return ActivityState.Failed;
    }

    if (!executionStatus) {
        return ActivityState.Pending;
    }

    return map[executionStatus] || ActivityState.Pending;
};

export const getTransactionAction = (
    transaction: IotaTransactionBlockResponse,
    currentAddress?: string,
) => {
    const isSender = transaction.transaction?.data.sender === currentAddress;
    return isSender ? ActivityAction.Transaction : ActivityAction.Receive;
};

export const getTransactionActivity = (
    transaction: IotaTransactionBlockResponse,
    address: string,
): Activity => {
    return {
        action: getTransactionAction(transaction, address),
        state: getTransactionActivityState(transaction),
        timestamp: transaction.timestampMs ? parseTimestamp(transaction.timestampMs) : undefined,
        transaction,
    };
};
