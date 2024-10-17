// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { IotaTransactionBlockResponse } from '@iota/iota-sdk/client';

// todo: add more logic for deriving transaction label
export const getLabel = (transaction: IotaTransactionBlockResponse, currentAddress?: string) => {
    const isSender = transaction.transaction?.data.sender === currentAddress;
    // Rename to "Send" to Transaction
    return isSender ? 'Transaction' : 'Receive';
};
