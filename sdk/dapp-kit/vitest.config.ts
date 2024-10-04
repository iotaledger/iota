// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// <reference types="vitest" />

import { config } from 'dotenv';
import { vanillaExtractPlugin } from '@vanilla-extract/vite-plugin';
import { defineConfig } from 'vite';
import { configDefaults } from 'vitest/config';

export default defineConfig({
    plugins: [vanillaExtractPlugin()],
    test: {
        exclude: [...configDefaults.exclude, 'tests/**'],
        environment: 'happy-dom',
        restoreMocks: true,
        globals: true,
        setupFiles: ['./test/setup.ts'],
        env: {
            ...config({ path: '../.env.defaults' }).parsed,
        },
    },
    resolve: {
        alias: {
            // TODO: Figure out a better way to run tests that avoids these aliases:
            '@iota/wallet-standard': new URL('../wallet-standard/src', import.meta.url).pathname,
            '@iota/bcs': new URL('../bcs/src', import.meta.url).pathname,
            '@iota/iota-sdk/keypairs/ed25519': new URL(
                '../typescript/src/keypairs/ed25519',
                import.meta.url,
            ).pathname,
            '@iota/iota-sdk/client': new URL('../typescript/src/client', import.meta.url).pathname,
            '@iota/iota-sdk/utils': new URL('../typescript/src/utils', import.meta.url).pathname,
            '@iota/iota-sdk/transactions': new URL(
                '../typescript/src/transactions',
                import.meta.url,
            ).pathname,
        },
    },
});
