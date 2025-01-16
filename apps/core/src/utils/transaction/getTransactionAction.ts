// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IotaTransactionBlockResponse } from '@iota/iota-sdk/client';
import { TransactionAction } from '../../interfaces';
import { checkIfIsTimelockedStaking } from '../stake';
import { isMigrationTransaction, isUnlockTimelockedObjectTransaction } from '..';

export const getTransactionAction = (
    transaction: IotaTransactionBlockResponse,
    currentAddress?: string,
) => {
    const sender = transaction.transaction?.data.sender;
    const {
        isTimelockedStaking,
        isTimelockedUnstaking,
        stakeTypeTransaction,
        unstakeTypeTransaction,
    } = checkIfIsTimelockedStaking(transaction?.events);

    const isMigration = isMigrationTransaction(transaction.transaction);
    const isSupplyIncreaseVestingCollect = isUnlockTimelockedObjectTransaction(
        transaction.transaction,
    );

    if (isMigration) {
        return TransactionAction.Migration;
    } else if (isSupplyIncreaseVestingCollect) {
        return TransactionAction.TimelockedCollect;
    } else if (stakeTypeTransaction) {
        return isTimelockedStaking ? TransactionAction.TimelockedStaked : TransactionAction.Staked;
    } else if (unstakeTypeTransaction) {
        return isTimelockedUnstaking
            ? TransactionAction.TimelockedUnstaked
            : TransactionAction.Unstaked;
    } else if (sender) {
        return sender === currentAddress ? TransactionAction.Send : TransactionAction.Receive;
    } else {
        return TransactionAction.Transaction;
    }
};
