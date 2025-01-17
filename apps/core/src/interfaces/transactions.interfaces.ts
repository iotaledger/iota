// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IotaTransactionBlockResponse } from '@iota/iota-sdk/client';

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
    Failed = 'Failed',
    Staked = 'Staked',
    Unstaked = 'Unstaked',
    TimelockedStaked = 'Timelocked Staked',
    TimelockedUnstaked = 'Timelocked Unstaked',
    TimelockedCollect = 'Timelocked Collect',
    Migration = 'Migration',
    Rewards = 'Rewards',
    PersonalMessage = 'PersonalMessage',
}

export enum TransactionState {
    Successful = 'successful',
    Failed = 'failed',
    Pending = 'pending',
}
