// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export interface Activity {
    action: ActivityAction;
    timestamp?: number;
    state: ActivityState;
}

export enum ActivityAction {
    Send = 'Send',
    Receive = 'Receive',
    Transaction = 'Transaction',
    Staked = 'Staked',
    Unstaked = 'Unstaked',
    Rewards = 'Rewards',
    PersonalMessage = 'PersonalMessage',
}

export enum ActivityState {
    Successful = 'successful',
    Failed = 'failed',
    Pending = 'pending',
}
