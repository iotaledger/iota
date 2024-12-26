// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    IotaTransactionBlockResponse,
    IotaTransactionBlockResponseOptions,
} from '@iota/iota-sdk/client';
import { Transaction } from '@iota/iota-sdk/transactions';

export type TransferAssetExecuteFn<T> = (input: {
    transaction: Transaction;
    options?: IotaTransactionBlockResponseOptions;
}) => Promise<T>;

export type WalletExecuteFn = (input: {
    transactionBlock: Uint8Array | Transaction;
    options?: IotaTransactionBlockResponseOptions;
}) => Promise<IotaTransactionBlockResponse>;

export type DappKitExecuteFn = (input: {
    transaction: Transaction | string;
    options?: IotaTransactionBlockResponseOptions;
}) => Promise<IotaTransactionBlockResponse>;
