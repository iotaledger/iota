// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export interface UID {
    id: string;
}

export interface Balance {
    value: number;
}

export interface Timelocked {
    id: UID;
    locked: Balance;
    expirationTimestampMs: number;
    label?: string;
}

export interface StakedIota {
    id: UID;
    poolId: string;
    stakeActivationEpoch: number;
    principal: Balance;
}

export interface TimelockedStakedIota {
    id: UID;
    stakedIota: StakedIota;
    expirationTimestampMs: number;
    label?: string;
}

export enum SupplyIncreaseUserType {
    Staker = 'Staker',
    Entity = 'Entity',
}

export interface SupplyIncreaseVestingPayout {
    amount: number;
    expirationTimestampMs: number;
}

export type SupplyIncreaseVestingPortfolio = SupplyIncreaseVestingPayout[];

export interface VestingOverview {
    totalVested: number;
    totalUnlocked: number;
    totalLocked: number;
    totalStaked: number;
    availableClaiming: number;
    availableStaking: number;
}
