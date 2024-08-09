// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { defineConfig } from 'vitest/config';
import { config } from 'dotenv';

export default defineConfig({
    test: {
        minThreads: 1,
        maxThreads: 8,
        hookTimeout: 1000000,
        testTimeout: 1000000,
        env: {
            NODE_ENV: 'test',
            ...config({ path: '../.env.defaults' }).parsed,
        },
    },
    resolve: {
        alias: {
            '@iota/bcs': new URL('../bcs/src', import.meta.url).toString(),
            '@iota/iota.js/transactions': new URL(
                '../typescript/src/transactions',
                import.meta.url,
            ).toString(),
            '@iota/iota.js': new URL('../typescript/src', import.meta.url).toString(),
        },
    },
});
