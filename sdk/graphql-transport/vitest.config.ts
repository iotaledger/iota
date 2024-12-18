// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { defineConfig } from 'vitest/config';

export default defineConfig({
	test: {
		minWorkers: 1,
		maxWorkers: 4,
		hookTimeout: 1000000,
		testTimeout: 1000000,
		env: {
			NODE_ENV: 'test',
		},
	},
	resolve: {
		alias: {
			'@iota/bcs': new URL('../bcs/src', import.meta.url).pathname,
			'@iota/iota-sdk/transactions': new URL('../typescript/src/transactions', import.meta.url)
				.pathname,
			'@iota/iota-sdk': new URL('../typescript/src', import.meta.url).pathname,
		},
	},
});
