// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IotaTransactionBlockResponse } from '@iota/iota-sdk/client';
import { TransactionAction } from '../../interfaces';
import { checkIfIsTimelockedStaking } from '../stake';
import { isMigrationTransaction, isUnlockTimelockedObjectTransaction } from '..';

export const ACTION_LABELS: Record<TransactionAction, string> = {
    [TransactionAction.Send]: 'Sent',
    [TransactionAction.Receive]: 'Received',
    [TransactionAction.Transaction]: 'Transaction',
    [TransactionAction.Staked]: 'Stake',
    [TransactionAction.Unstaked]: 'Unstake',
    [TransactionAction.TimelockedStaked]: 'Stake Vesting',
    [TransactionAction.TimelockedUnstaked]: 'Unstake Vesting',
    [TransactionAction.TimelockedCollect]: 'Collect Vesting',
    [TransactionAction.Migration]: 'Migration',
    [TransactionAction.PersonalMessage]: 'Personal Message',
};

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
    const isVestingCollect = isUnlockTimelockedObjectTransaction(transaction.transaction);

    // A coin transfer only has coin-related commands (no contract calls, publish or upgrade)
    const programmableTx = transaction.transaction?.data.transaction;
    const isCoinTransfer =
        programmableTx?.kind === 'ProgrammableTransaction' &&
        !programmableTx.transactions.some(
            (cmd) => 'MoveCall' in cmd || 'Publish' in cmd || 'Upgrade' in cmd,
        );

    if (isMigration) {
        return TransactionAction.Migration;
    } else if (isVestingCollect) {
        return TransactionAction.TimelockedCollect;
    } else if (stakeTypeTransaction) {
        return isTimelockedStaking ? TransactionAction.TimelockedStaked : TransactionAction.Staked;
    } else if (unstakeTypeTransaction) {
        return isTimelockedUnstaking
            ? TransactionAction.TimelockedUnstaked
            : TransactionAction.Unstaked;
    } else if (sender && isCoinTransfer) {
        return sender === currentAddress ? TransactionAction.Send : TransactionAction.Receive;
    } else {
        return TransactionAction.Transaction;
    }
};
