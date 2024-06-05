// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SuiTransactionBlockResponse } from '@mysten/sui.js/client';

export interface Activity {
    action: ActivityAction;
    timestamp?: number;
    state: ActivityState;
    transaction: SuiTransactionBlockResponse;
}

export enum ActivityAction {
    Send = 'Send',
    Receive = 'Receive',
    Transaction = 'Transaction',
    Staked = 'Staked',
    Unstaked = 'Unstaked',
    Rewards = 'Rewards',
    Swapped = 'Swapped',
    PersonalMessage = 'PersonalMessage',
}

export enum ActivityState {
    Successful = 'successful',
    Failed = 'failed',
    Pending = 'pending',
}
