// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { isIotaNSName, useIotaNSEnabled } from '@iota/core';
import { useIotaClient } from '@iota/dapp-kit';
import { type IotaClient } from '@iota/iota.js/client';
import { isValidIotaAddress } from '@iota/iota.js/utils';
import { useMemo } from 'react';
import * as Yup from 'yup';

export function createIotaAddressValidation(client: IotaClient, iotaNSEnabled: boolean) {
    const resolveCache = new Map<string, boolean>();

    return Yup.string()
        .ensure()
        .trim()
        .required()
        .test('is-iota-address', 'Invalid address. Please check again.', async (value) => {
            if (iotaNSEnabled && isIotaNSName(value)) {
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

export function useIotaAddressValidation() {
    const client = useIotaClient();
    const iotaNSEnabled = useIotaNSEnabled();

    return useMemo(() => {
        return createIotaAddressValidation(client, iotaNSEnabled);
    }, [client, iotaNSEnabled]);
}
