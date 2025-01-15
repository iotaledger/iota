// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type {
    IotaTransaction,
    IotaTransactionBlockResponse,
    MoveCallIotaTransaction,
} from '@iota/iota-sdk/client';
import { STARDUST_PACKAGE_ID } from '../../constants';

export function checkIfIsMigrationTransaction(
    transaction: IotaTransactionBlockResponse['transaction'],
) {
    if (!transaction || transaction.data.transaction.kind !== 'ProgrammableTransaction')
        return { isMigration: false };
    const moveCallTxs = transaction.data.transaction.transactions.filter(isMoveCall);
    const isMigration = moveCallTxs.some(
        (tx) =>
            tx.MoveCall.package === STARDUST_PACKAGE_ID &&
            tx.MoveCall.function === 'extract_assets',
    );

    return {
        isMigration,
    };
}

function isMoveCall(
    transaction: IotaTransaction,
): transaction is { MoveCall: MoveCallIotaTransaction } {
    return 'MoveCall' in transaction;
}
