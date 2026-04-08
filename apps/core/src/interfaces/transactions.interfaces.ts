// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export enum TransactionAction {
    Send = 'Send',
    Receive = 'Receive',
    Transaction = 'Transaction',
    Staked = 'Stake',
    Unstaked = 'Unstake',
    TimelockedStaked = 'Stake Vesting',
    TimelockedUnstaked = 'Unstake Vesting',
    TimelockedCollect = 'Collect Vesting',
    Migration = 'Migration',
    PersonalMessage = 'Personal Message',
}
