// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { StakeEventJson } from '../../interfaces';

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
