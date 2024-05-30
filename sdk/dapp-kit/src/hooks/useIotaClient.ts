// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { IotaClient } from '@mysten/iota.js/client';
import { useContext } from 'react';

import { IotaClientContext } from '../components/IotaClientProvider.js';

export function useIotaClientContext() {
	const iotaClient = useContext(IotaClientContext);

	if (!iotaClient) {
		throw new Error(
			'Could not find IotaClientContext. Ensure that you have set up the IotaClientProvider',
		);
	}

	return iotaClient;
}

export function useIotaClient(): IotaClient {
	return useIotaClientContext().client;
}
