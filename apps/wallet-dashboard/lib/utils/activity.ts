// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Activity, ActivityAction, ActivityState } from '@/lib/interfaces';
import { ExecutionStatus, SuiTransactionBlockResponse } from '@mysten/sui.js/client';

const executionStatusToActivityState = (tx: SuiTransactionBlockResponse): ActivityState => {
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

export const txGetAction = (transaction: SuiTransactionBlockResponse, currentAddress?: string) => {
    const isSender = transaction.transaction?.data.sender === currentAddress;
    return isSender ? ActivityAction.Transaction : ActivityAction.Receive;
};

const txGetTimestamp = (timestampMs?: string | null): number | undefined => {
    if (!timestampMs) {
        return;
    }

    const timestamp = new Date(parseInt(timestampMs)).getTime();

    if (isNaN(timestamp)) {
        return;
    }

    return timestamp;
};

export const txToActivity = (
    tx: SuiTransactionBlockResponse,
    address?: string,
): Activity | undefined => {
    if (!address) {
        return;
    }

    return {
        action: txGetAction(tx, address),
        state: executionStatusToActivityState(tx),
        timestamp: txGetTimestamp(tx.timestampMs),
    };
};
