// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { UNSTAKING_REQUEST_EVENT } from '../../constants';
import { UnstakeEventJson } from '../../interfaces';
import type { IotaEvent } from '@iota/iota-sdk/client';

export function getUnstakeDetailsFromEvents(events: IotaEvent[]): {
    principalAmount: string;
    rewardAmount: string;
    validatorAddress: string;
    totalAmountWithoutRewards: bigint;
    totalUnstakeAmount: bigint;
    totalRewards: bigint;
} {
    const unstakeEvent = events.find(({ type }) => type === UNSTAKING_REQUEST_EVENT);
    const eventJson = unstakeEvent?.parsedJson as UnstakeEventJson;

    const totalAmountWithoutRewards = events?.reduce((sum, event) => {
        return (
            sum + Number((event.parsedJson as { principal_amount: number }).principal_amount || 0)
        );
    }, 0);

    const totalRewards = events?.reduce((sum, event) => {
        return sum + Number((event.parsedJson as { reward_amount: number }).reward_amount || 0);
    }, 0);

    const totalUnstakeAmount = BigInt(totalAmountWithoutRewards) + BigInt(totalRewards);

    return {
        principalAmount: eventJson.principal_amount || '0',
        rewardAmount: eventJson.reward_amount || '0',
        validatorAddress: eventJson.validator_address || '',
        totalAmountWithoutRewards: BigInt(totalAmountWithoutRewards),
        totalRewards: BigInt(totalRewards),
        totalUnstakeAmount,
    };
}
