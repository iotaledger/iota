// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type IotaEvent } from '@iota/iota-sdk/client';

export function getValidatorMoveEvent(
    validatorsEvent: IotaEvent[],
    validatorAddress: string,
    currentEpoch?: string,
): IotaEvent | undefined | unknown {
    const event = validatorsEvent.find(({ parsedJson }) => {
        const parsed = parsedJson as { validator_address?: string; epoch?: string };
        return (
            currentEpoch &&
            parsed.epoch === currentEpoch &&
            parsed.validator_address === validatorAddress
        );
    });

    return event && event.parsedJson;
}
