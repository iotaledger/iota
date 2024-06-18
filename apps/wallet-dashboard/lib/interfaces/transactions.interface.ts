// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IotaTransactionBlockResponse } from '@iota/iota.js/client';

export interface ExtendedTransaction {
    action: TransactionAction;
    timestamp?: number;
    state: TransactionState;
    raw: IotaTransactionBlockResponse;
}

export enum TransactionAction {
    Send = 'Send',
    Receive = 'Receive',
    Transaction = 'Transaction',
    Staked = 'Staked',
    Unstaked = 'Unstaked',
    Rewards = 'Rewards',
    PersonalMessage = 'PersonalMessage',
}

export enum TransactionState {
    Successful = 'successful',
    Failed = 'failed',
    Pending = 'pending',
}
