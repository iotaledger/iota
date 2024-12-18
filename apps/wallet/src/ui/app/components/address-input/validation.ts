// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaNSEnabled } from '@iota/core';
import { useIotaClient } from '@iota/dapp-kit';
import { type IotaClient } from '@iota/iota-sdk/client';
import { isValidIotaAddress, isValidIotaNSName } from '@iota/iota-sdk/utils';
import { useMemo } from 'react';
import * as Yup from 'yup';

const CACHE_EXPIRY_TIME = 60 * 1000; // 1 minute in milliseconds

export function createIotaAddressValidation(client: IotaClient, iotaNSEnabled: boolean) {
	const resolveCache = new Map<string, { valid: boolean; expiry: number }>();

	const currentTime = Date.now();
	return Yup.string()
		.ensure()
		.trim()
		.required()
		.test('is-iota-address', 'Invalid address. Please check again.', async (value) => {
			if (iotaNSEnabled && isValidIotaNSName(value)) {
				if (resolveCache.has(value)) {
					const cachedEntry = resolveCache.get(value)!;
					if (currentTime < cachedEntry.expiry) {
						return cachedEntry.valid;
					} else {
						resolveCache.delete(value); // Remove expired entry
					}
				}

				const address = await client.resolveNameServiceAddress({
					name: value,
				});

				resolveCache.set(value, {
					valid: !!address,
					expiry: currentTime + CACHE_EXPIRY_TIME,
				});

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
