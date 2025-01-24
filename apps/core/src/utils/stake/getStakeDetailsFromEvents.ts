// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { STAKING_REQUEST_EVENT } from '../../constants';
import { StakeEventJson } from '../../interfaces';
import type { IotaEvent } from '@iota/iota-sdk/client';

export function getStakeDetailsFromEvents(events: IotaEvent[]): {
    totalStakedAmount: string;
    validatorAddress: string;
    epoch: number;
} {
    const stakeEvent = events.find((event) => event.type === STAKING_REQUEST_EVENT);

    const eventJson = stakeEvent?.parsedJson as StakeEventJson;
    const totalStakedAmount = events?.reduce((sum, event) => {
        return sum + Number((event.parsedJson as { amount: number }).amount || 0);
    }, 0);
    return {
        totalStakedAmount: totalStakedAmount.toString(),
        validatorAddress: eventJson.validator_address || '',
        epoch: Number(eventJson.epoch || '0'),
    };
}
