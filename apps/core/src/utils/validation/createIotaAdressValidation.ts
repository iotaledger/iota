// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IotaClient } from '@iota/iota.js/client';
import { isIotaNSName } from '../../hooks';
import { isValidIotaAddress } from '@iota/iota.js/utils';
import * as Yup from 'yup';

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
