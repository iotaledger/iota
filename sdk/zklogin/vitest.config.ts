// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { defineConfig } from 'vitest/config';

export default defineConfig({
	resolve: {
		alias: {
			'@mysten/bcs': new URL('../bcs/src', import.meta.url).toString(),
			'@mysten/sui.js': new URL('../typescript/src', import.meta.url).toString(),
		},
	},
});
