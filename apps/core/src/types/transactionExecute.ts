// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    IotaTransactionBlockResponse,
    IotaTransactionBlockResponseOptions,
} from '@iota/iota-sdk/client';
import { Transaction } from '@iota/iota-sdk/transactions';

export type TransferAssetExecuteFn = (input: {
    transaction: Transaction;
    options?: IotaTransactionBlockResponseOptions;
}) => Promise<IotaTransactionBlockResponse>;
