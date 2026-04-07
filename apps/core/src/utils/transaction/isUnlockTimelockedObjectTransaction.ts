// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type {
    IotaTransaction,
    IotaTransactionBlockResponse,
    MoveCallIotaTransaction,
} from '@iota/iota-sdk/client';
import {
    TIMELOCK_MODULE,
    TIMELOCK_PACKAGE_ID,
    TIMELOCKED_STAKING_MODULE,
    TIMELOCKED_STAKING_PACKAGE_ID,
} from '../..';

export function isUnlockTimelockedObjectTransaction(
    transaction: IotaTransactionBlockResponse['transaction'],
): boolean {
    if (!transaction || transaction.data.transaction.kind !== 'ProgrammableTransaction')
        return false;
    const moveCallTxs = transaction.data.transaction.transactions.filter(isMoveCall);

    // Unlock timelocked objects: function unlock or unlock_with_clock
    const hasTimelockUnlock = moveCallTxs.some(
        (tx) =>
            tx.MoveCall.package === TIMELOCK_PACKAGE_ID &&
            tx.MoveCall.module === TIMELOCK_MODULE &&
            (tx.MoveCall.function === 'unlock' || tx.MoveCall.function === 'unlock_with_clock'),
    );

    // Re-stake timelocked objects
    const hasTimelockedStaking = moveCallTxs.some(
        (tx) =>
            tx.MoveCall.package === TIMELOCKED_STAKING_PACKAGE_ID &&
            tx.MoveCall.module === TIMELOCKED_STAKING_MODULE,
    );

    return hasTimelockUnlock || hasTimelockedStaking;
}

function isMoveCall(
    transaction: IotaTransaction,
): transaction is { MoveCall: MoveCallIotaTransaction } {
    return 'MoveCall' in transaction;
}
