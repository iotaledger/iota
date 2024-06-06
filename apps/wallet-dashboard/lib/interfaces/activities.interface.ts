// Copyright (c) 2024 IOTA Stiftung
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export interface Activity {
    action:
        | 'Send'
        | 'Receive'
        | 'Transaction'
        | 'Staked'
        | 'Unstaked'
        | 'Rewards'
        | 'Swapped'
        | 'PersonalMessage';
    timestamp: number;
    state: ActivityState;
}

export enum ActivityState {
    Successful = 'successful',
    Failed = 'failed',
    Pending = 'pending',
}
