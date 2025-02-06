// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// <reference types="vitest" />
import react from '@vitejs/plugin-react';
import { execSync } from 'child_process';
import { defineConfig } from 'vite';
import svgr from 'vite-plugin-svgr';
import { configDefaults } from 'vitest/config';

process.env.VITE_VERCEL_ENV = process.env.VERCEL_ENV || 'development';
process.env.VITE_BUILD_ENV = process.env.BUILD_ENV || 'development';
const EXPLORER_REV = execSync('git rev-parse HEAD').toString().trim().toString();

// https://vitejs.dev/config/
export default defineConfig({
    plugins: [react(), svgr()],
    test: {
        // Omit end-to-end tests:
        exclude: [...configDefaults.exclude, 'tests/**'],
        css: true,
        globals: true,
        environment: 'happy-dom',
    },
    build: {
        // Set the output directory to match what CRA uses:
        outDir: 'build',
        sourcemap: true,
    },
    resolve: {
        alias: {
            '~': new URL('./src', import.meta.url).pathname,
        },
    },
    define: {
        EXPLORER_REV: JSON.stringify(EXPLORER_REV),
    },
});
