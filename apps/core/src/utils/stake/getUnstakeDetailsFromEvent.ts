// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { UnstakeEventJson } from '../../interfaces';
import type { IotaEvent } from '@iota/iota-sdk/client';

export function getUnstakeDetailsFromEvent(event: IotaEvent): {
    principalAmount: string;
    rewardAmount: string;
    totalAmount: bigint;
    validatorAddress: string;
} {
    const eventJson = event.parsedJson as UnstakeEventJson;
    const principalAmount = eventJson.principal_amount || '0';
    const rewardAmount = eventJson.reward_amount || '0';
    const totalAmount = BigInt(principalAmount) + BigInt(rewardAmount);
    return {
        principalAmount,
        rewardAmount,
        totalAmount,
        validatorAddress: eventJson.validator_address || '',
    };
}
