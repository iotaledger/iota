// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { config } from 'dotenv';
import { defineConfig } from 'vitest/config';

export default defineConfig({
    test: {
        minThreads: 1,
        maxThreads: 8,
        hookTimeout: 1000000,
        testTimeout: 1000000,
        env: {
            ...config({ path: '../.env.defaults' }).parsed,
        },
    },
    resolve: {
        alias: {
            '@iota/bcs': new URL('../bcs/src', import.meta.url).toString(),
            '@iota/iota-sdk': new URL('../typescript/src', import.meta.url).toString(),
        },
    },
});
