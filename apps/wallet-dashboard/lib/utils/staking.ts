// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { IotaEvent, IotaTransactionBlockResponse } from '@iota/iota-sdk/client';
import { ParsedJson, StakeEventJson, UnstakeEventJson } from '../interfaces';
import { STAKING_REQUEST_EVENT, UNSTAKING_REQUEST_EVENT } from '@iota/core';

export function checkIfIsTimelockedStaking(events: IotaTransactionBlockResponse['events']) {
    const TIMELOCKED_STAKING_EVENT_MODULE = 'timelocked_staking';
    if (!events) return { isTimelockedStaking: false, isTimelockedUnstaking: false };

    const stakeTypeTransaction = events?.find(({ type }) => type === STAKING_REQUEST_EVENT);
    const unstakeTypeTransaction = events?.find(({ type }) => type === UNSTAKING_REQUEST_EVENT);

    const isTimelockedStaking =
        stakeTypeTransaction?.transactionModule === TIMELOCKED_STAKING_EVENT_MODULE;
    const isTimelockedUnstaking =
        unstakeTypeTransaction?.transactionModule === TIMELOCKED_STAKING_EVENT_MODULE;

    return {
        isTimelockedStaking,
        isTimelockedUnstaking,
        stakeTypeTransaction,
        unstakeTypeTransaction,
    };
}

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
