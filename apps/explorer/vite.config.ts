// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// <reference types="vitest" />
import { sentryVitePlugin } from '@sentry/vite-plugin';
import react from '@vitejs/plugin-react';
import { execSync } from 'child_process';
import { copyFileSync } from 'fs';
import { defineConfig, loadEnv } from 'vite';
import svgr from 'vite-plugin-svgr';
import { configDefaults } from 'vitest/config';

process.env.VITE_VERCEL_ENV = process.env.VERCEL_ENV || 'development';
const EXPLORER_REV = execSync('git rev-parse HEAD').toString().trim().toString();

// https://vitejs.dev/config/
export default defineConfig(({ mode }) => {
    const env = loadEnv(mode, process.cwd(), '');
    const sentryAuthToken = env.SENTRY_AUTH_TOKEN;
    const IS_PROD = env.VITE_BUILD_ENV === 'production';
    return {
        plugins: [
            react(),
            svgr(),
            sentryVitePlugin({
                org: 'iota-foundation-eu',
                project: 'iota-explorer',
                authToken: sentryAuthToken,
                sourcemaps: {
                    assets: './build/**',
                },
                disable: !IS_PROD || !sentryAuthToken,
                silent: !env.CI,
                release: {
                    name: EXPLORER_REV,
                },
            }),
            {
                name: 'copy-wasm-files',
                buildStart() {
                    // Copy WASM files to public directory
                    try {
                        copyFileSync(
                            'node_modules/@iota/identity-wasm/web/identity_wasm_bg.wasm',
                            'public/identity_wasm_bg.wasm',
                        );
                    } catch (error) {
                        console.warn('Could not copy WASM files:', error);
                    }
                },
            },
        ],
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
            'process.env.APPS_BACKEND': JSON.stringify(process.env.APPS_BACKEND ?? ''),
        },
    };
});
