// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { UNSTAKING_REQUEST_EVENT } from '../../constants';
import { UnstakeEventJson } from '../../interfaces';
import type { IotaEvent } from '@iota/iota-sdk/client';

export function getUnstakeDetailsFromEvents(events: IotaEvent[]): {
    validatorAddress: string;
    unstakeAmount: bigint;
    totalUnstakeAmount: bigint;
    unstakeRewards: bigint;
} {
    const unstakeEvent = events.find(({ type }) => type === UNSTAKING_REQUEST_EVENT);
    const unstakeEventJson = unstakeEvent?.parsedJson as UnstakeEventJson;

    const unstakeAmount = events?.reduce((sum, event) => {
        return (
            sum + Number((event.parsedJson as { principal_amount: number }).principal_amount || 0)
        );
    }, 0);

    const unstakeRewards = events?.reduce((sum, event) => {
        return sum + Number((event.parsedJson as { reward_amount: number }).reward_amount || 0);
    }, 0);

    const totalUnstakeAmount = BigInt(unstakeAmount) + BigInt(unstakeRewards);

    return {
        validatorAddress: unstakeEventJson.validator_address || '',
        unstakeAmount: BigInt(unstakeAmount),
        unstakeRewards: BigInt(unstakeRewards),
        totalUnstakeAmount,
    };
}
