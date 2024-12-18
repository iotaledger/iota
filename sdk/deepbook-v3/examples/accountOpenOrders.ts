// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { getFullnodeUrl, IotaClient } from '@iota/iota-sdk/client';

import { DeepBookClient } from '../src/index.js'; // Adjust import source accordingly

/// Example to get open orders for a balance manager for all pools
(async () => {
	const env = 'mainnet';

	const balanceManagers = {
		MANAGER_1: {
			address: '0x344c2734b1d211bd15212bfb7847c66a3b18803f3f5ab00f5ff6f87b6fe6d27d',
			tradeCap: '',
		},
	};

	const dbClient = new DeepBookClient({
		address: '0x0',
		env: env,
		client: new IotaClient({
			url: getFullnodeUrl(env),
		}),
		balanceManagers: balanceManagers,
	});

	const manager = 'MANAGER_1'; // Update the manager accordingly
	const pools = ['IOTA_USDC', 'DEEP_IOTA', 'DEEP_USDC', 'WUSDT_USDC', 'WUSDC_USDC', 'BETH_USDC']; // Live pools, add more if needed
	console.log('Manager:', manager);

	for (const pool of pools) {
		console.log(pool);
		console.log(await dbClient.accountOpenOrders(pool, manager));
	}
})();
