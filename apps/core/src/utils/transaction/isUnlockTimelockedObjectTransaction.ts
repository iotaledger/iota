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

function isMoveCall(
    transaction: IotaTransaction,
): transaction is { MoveCall: MoveCallIotaTransaction } {
    return 'MoveCall' in transaction;
}
