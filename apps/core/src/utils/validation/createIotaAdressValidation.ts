// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IotaClient } from '@iota/iota-sdk/client';
import { isIotaNSName } from '../../hooks';
import { isValidIotaAddress } from '@iota/iota-sdk/utils';
import * as Yup from 'yup';
import { ValidationError } from 'yup';

export { ValidationError };

export function createIotaAddressValidation(client?: IotaClient, iotaNSEnabled?: boolean) {
    const resolveCache = new Map<string, boolean>();

    return Yup.string()
        .ensure()
        .trim()
        .required()
        .test('is-iota-address', 'Invalid address. Please check again.', async (value) => {
            if (client && iotaNSEnabled && isIotaNSName(value)) {
                if (resolveCache.has(value)) {
                    return resolveCache.get(value)!;
                }

                const address = await client.resolveNameServiceAddress({
                    name: value,
                });

                resolveCache.set(value, !!address);

                return !!address;
            }

            return isValidIotaAddress(value);
        })
        .label("Recipient's address");
}
