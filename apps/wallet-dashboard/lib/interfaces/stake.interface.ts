// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export interface Stake {
    id: string;
    validator: string;
    stake: string;
    rewards: string;
    stakeActiveEpoch?: string;
    stakeRequestEpoch?: string;
    estimatedReward?: string;
    status: string;
}
