// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { IotaEvent } from '@iota/iota-sdk/client';
import { ParsedJson, StakeEventJson, UnstakeEventJson } from '../interfaces';
import { checkIfIsTimelockedStaking } from '@iota/core';

export function parseEventJson<T extends ParsedJson>(event: IotaEvent): T {
    return event.parsedJson as T;
}

export function getStakeDetails(eventJson: StakeEventJson): {
    stakedAmount: bigint;
    validatorAddress: string;
    epoch: number;
} {
    return {
        stakedAmount: BigInt(eventJson.amount || '0'),
        validatorAddress: eventJson.validator_address || '',
        epoch: Number(eventJson.epoch || '0'),
    };
}

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

export function getTransactionAmountForTimelocked(events: IotaEvent[]): bigint | undefined {
    if (!events) return;
    const { isTimelockedStaking, isTimelockedUnstaking } = checkIfIsTimelockedStaking(events);

    if (isTimelockedStaking) {
        const eventJson = parseEventJson<StakeEventJson>(events[0]);
        const { stakedAmount } = getStakeDetails(eventJson);
        return stakedAmount;
    } else if (isTimelockedUnstaking) {
        const eventJson = parseEventJson<UnstakeEventJson>(events[0]);
        const { totalAmount } = getUnstakeDetails(eventJson);
        return totalAmount;
    }
}
