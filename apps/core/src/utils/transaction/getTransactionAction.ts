// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IotaTransactionBlockResponse } from '@iota/iota-sdk/client';
import { TransactionAction } from '../../interfaces';

export const getTransactionAction = (
    transaction: IotaTransactionBlockResponse,
    currentAddress?: string,
) => {
    const isSender = transaction.transaction?.data.sender === currentAddress;
    return isSender ? TransactionAction.Send : TransactionAction.Receive;
};
