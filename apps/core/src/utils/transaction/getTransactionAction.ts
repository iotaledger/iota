// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IotaTransactionBlockResponse } from '@iota/iota-sdk/client';
import { TransactionAction } from '../../interfaces';
import { STAKING_REQUEST_EVENT, UNSTAKING_REQUEST_EVENT } from '../../constants';

export const getTransactionAction = (
    transaction: IotaTransactionBlockResponse,
    currentAddress?: string,
) => {
    const stakeTypeTransaction = transaction?.events?.find(
        ({ type }) => type === STAKING_REQUEST_EVENT,
    );
    const unstakeTypeTransaction = transaction?.events?.find(
        ({ type }) => type === UNSTAKING_REQUEST_EVENT,
    );
    if (stakeTypeTransaction) {
        return TransactionAction.Staked;
    } else if (unstakeTypeTransaction) {
        return TransactionAction.Unstaked;
    } else {
        const isSender = transaction.transaction?.data.sender === currentAddress;
        return isSender ? TransactionAction.Send : TransactionAction.Receive;
    }
};
