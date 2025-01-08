// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IotaTransactionBlockResponse } from '@iota/iota-sdk/client';
import { TransactionAction } from '../../interfaces';
import { checkIfIsTimelockedStaking } from '../stake/checkIfIsTimelockedStaking';

export const getTransactionAction = (
    transaction: IotaTransactionBlockResponse,
    currentAddress?: string,
) => {
    const {
        isTimelockedStaking,
        isTimelockedUnstaking,
        stakeTypeTransaction,
        unstakeTypeTransaction,
    } = checkIfIsTimelockedStaking(transaction?.events);

    if (stakeTypeTransaction) {
        return isTimelockedStaking ? TransactionAction.TimelockedStaked : TransactionAction.Staked;
    } else if (unstakeTypeTransaction) {
        return isTimelockedUnstaking
            ? TransactionAction.TimelockedUnstaked
            : TransactionAction.Unstaked;
    } else {
        const isSender = transaction.transaction?.data.sender === currentAddress;
        return isSender ? TransactionAction.Transaction : TransactionAction.Receive;
    }
};
