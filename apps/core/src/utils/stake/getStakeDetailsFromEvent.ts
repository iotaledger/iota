// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { StakeEventJson } from '../../interfaces';
import type { IotaEvent } from '@iota/iota-sdk/client';

export function getStakeDetailsFromEvent(event: IotaEvent): {
    stakedAmount: string;
    validatorAddress: string;
    epoch: number;
} {
    const eventJson = event.parsedJson as StakeEventJson;
    return {
        stakedAmount: eventJson.amount || '0',
        validatorAddress: eventJson.validator_address || '',
        epoch: Number(eventJson.epoch || '0'),
    };
}
