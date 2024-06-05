// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Activity, ActivityAction, ActivityState } from '@/lib/interfaces';
import { ExecutionStatus, SuiTransactionBlockResponse } from '@mysten/sui.js/client';
import { parseTimestamp } from './time';

const getTransactionActivityState = (tx: SuiTransactionBlockResponse): ActivityState => {
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
    transaction: SuiTransactionBlockResponse,
    currentAddress?: string,
) => {
    const isSender = transaction.transaction?.data.sender === currentAddress;
    return isSender ? ActivityAction.Transaction : ActivityAction.Receive;
};

export const getTransactionActivity = (
    transaction: SuiTransactionBlockResponse,
    address: string,
): Activity => {
    return {
        action: getTransactionAction(transaction, address),
        state: getTransactionActivityState(transaction),
        timestamp: parseTimestamp(transaction.timestampMs),
        transaction,
    };
};
