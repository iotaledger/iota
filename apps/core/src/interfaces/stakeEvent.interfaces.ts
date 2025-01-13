// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export interface StakeEventJson {
    amount: string;
    validator_address: string;
    epoch: string;
}

export interface UnstakeEventJson {
    principal_amount?: string;
    reward_amount?: string;
    validator_address?: string;
}
