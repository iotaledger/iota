// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type {
    IotaTransaction,
    IotaTransactionBlockResponse,
    MoveCallIotaTransaction,
} from '@iota/iota-sdk/client';
import { TIMELOCK_MODULE } from '../../';

export function checkIfIsSupplyIncreaseVestingCollectTransaction(
    transaction: IotaTransactionBlockResponse['transaction'],
) {
    if (!transaction || transaction.data.transaction.kind !== 'ProgrammableTransaction')
        return { isSupplyIncreaseVestingCollect: false };
    const moveCallTxs = transaction.data.transaction.transactions
        .filter(isMoveCall)
        .filter((tx) => tx.MoveCall.module === TIMELOCK_MODULE);
    const isSupplyIncreaseVestingCollect =
        moveCallTxs.length > 0 && moveCallTxs.every((tx) => tx.MoveCall.function === 'unlock');

    return {
        isSupplyIncreaseVestingCollect,
    };
}

function isMoveCall(
    transaction: IotaTransaction,
): transaction is { MoveCall: MoveCallIotaTransaction } {
    return 'MoveCall' in transaction;
}
