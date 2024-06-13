// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

type BaseStakeType = {
    validator: string;
    stake: string;
    stakeActiveEpoch: string;
    stakeRequestEpoch: string;
    stakedIotaId: string;
};

type PendingStakeType = BaseStakeType & {
    status: 'Pending';
};

type ActiveStakeType = BaseStakeType & {
    estimatedReward: string;
    status: 'Active';
};

type UnstakedStakeType = BaseStakeType & {
    status: 'Unstaked';
};

export type Stake = BaseStakeType & (PendingStakeType | ActiveStakeType | UnstakedStakeType);
