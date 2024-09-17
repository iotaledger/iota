// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type IotaClient } from '@iota/iota-sdk/client';
import * as Yup from 'yup';
import { createIotaAddressValidation } from '../validation';
import { ValidationError } from 'yup';

export function createNftSendValidationSchema(
    senderAddress: string,
    objectId: string,
    client?: IotaClient,
    iotaNSEnabled?: boolean,
) {
    return Yup.object({
        to: createIotaAddressValidation(client, iotaNSEnabled)
            .test(
                'sender-address',
                'NFT is owned by this address',
                (value) => senderAddress !== value,
            )
            .test(
                'nft-sender-address',
                'NFT address must be different from receiver address',
                (value) => objectId !== value,
            ),
    });
}

export { ValidationError };
