// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type IotaEvent } from '@iota/iota-sdk/client';

export function getValidatorMoveEvent(
    validatorsEvent: IotaEvent[],
    validatorAddress: string,
): IotaEvent | undefined | unknown {
    const event = validatorsEvent.find(
        ({ parsedJson }) =>
            (parsedJson as { validator_address?: unknown })!.validator_address === validatorAddress,
    );

    return event && event.parsedJson;
}
