// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export enum SupplyIncreaseUserType {
    Staker = 'Staker',
    Entity = 'Entity',
}

export interface SupplyIncreaseVestingPayout {
    amount: bigint;
    expirationTimestampMs: number;
}

export type SupplyIncreaseVestingPortfolio = SupplyIncreaseVestingPayout[];

export interface VestingOverview {
    totalVested: bigint;
    totalUnlocked: bigint;
    totalLocked: bigint;
    totalStaked: bigint;
    totalEarned: bigint;
    availableClaiming: bigint;
    availableStaking: bigint;
}
