// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type {
    IotaTransaction,
    IotaTransactionBlockResponse,
    MoveCallIotaTransaction,
} from '@iota/iota-sdk/client';
import { TIMELOCK_MODULE } from '../..';

export function isUnlockTimelockedObjectTransaction(
    transaction: IotaTransactionBlockResponse['transaction'],
): boolean {
    if (!transaction || transaction.data.transaction.kind !== 'ProgrammableTransaction')
        return false;
    const moveCallTxs = transaction.data.transaction.transactions
        .filter(isMoveCall)
        .filter((tx) => tx.MoveCall.module === TIMELOCK_MODULE);
    const isUnlockTimelockedObject =
        moveCallTxs.length > 0 && moveCallTxs.every((tx) => tx.MoveCall.function === 'unlock');
    return isUnlockTimelockedObject;
}

// Detects a collect-all-vesting/timelocks transaction (MoveCall to unlock_with_clock)
export function isCollectAllTimelocksTransaction(
    transaction: IotaTransactionBlockResponse['transaction'],
): boolean {
    if (!transaction || transaction.data.transaction.kind !== 'ProgrammableTransaction')
        return false;
    const moveCallTxs = transaction.data.transaction.transactions
        .filter(isMoveCall)
        .filter((tx) => tx.MoveCall.module === TIMELOCK_MODULE);
    // At least one call to unlock_with_clock
    const isCollectAll =
        moveCallTxs.length > 0 && moveCallTxs.some((tx) => tx.MoveCall.function === 'unlock_with_clock');
    return isCollectAll;
}

function isMoveCall(
    transaction: IotaTransaction,
): transaction is { MoveCall: MoveCallIotaTransaction } {
    return 'MoveCall' in transaction;
}
