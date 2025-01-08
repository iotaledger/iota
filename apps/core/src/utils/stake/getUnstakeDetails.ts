// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { UnstakeEventJson } from '../../interfaces';

export function getUnstakeDetails(eventJson: UnstakeEventJson): {
    principalAmount: bigint;
    rewardAmount: bigint;
    totalAmount: bigint;
    validatorAddress: string;
} {
    const principalAmount = BigInt(eventJson.principal_amount || '0');
    const rewardAmount = BigInt(eventJson.reward_amount || '0');
    return {
        principalAmount,
        rewardAmount,
        totalAmount: principalAmount + rewardAmount,
        validatorAddress: eventJson.validator_address || '',
    };
}
