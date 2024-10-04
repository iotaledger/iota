// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

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
