// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/* eslint-disable no-empty-pattern */

import path from 'path';
import { test as base, chromium, Page, type BrowserContext } from '@playwright/test';

import { createPage } from './browser';

// Path to the IOTA Wallet extension (downloaded via `pnpm run test:prepare`)
const EXTENSION_PATH = path.join(__dirname, '../../wallet-dist');

const COMMON_ARGS = ['--user-agent=Playwright', '--disable-dev-shm-usage', '--no-sandbox'];

const DEFAULT_SHARED_STATE = { extension: {}, wallet: {} };

export interface SharedState {
    sharedContext?: BrowserContext;
    extension: {
        url?: string;
        id?: string;
        name?: string;
    };
    wallet: {
        address?: string;
        mnemonic?: string;
        page?: Page;
    };
}

const sharedState: SharedState = { ...DEFAULT_SHARED_STATE };

/**
 * Extended test with IOTA Wallet extension support
 */
export const test = base.extend<{
    sharedState: SharedState;
    extensionUrl: string;
    extensionId: string;
    appPage: Page;
    extensionPage: Page;
    extensionName: string;
}>({
    sharedState: async ({}, use) => {
        await use(sharedState);
    },

    context: [
        async ({ sharedState }, use) => {
            const isCI = !!process.env.CI;

            if (sharedState.sharedContext) {
                await use(sharedState.sharedContext);
                return;
            }

            const context = await chromium.launchPersistentContext('', {
                headless: false,
                viewport: { width: 1440, height: 900 },
                args: [
                    ...COMMON_ARGS,
                    `--disable-extensions-except=${EXTENSION_PATH}`,
                    `--load-extension=${EXTENSION_PATH}`,
                    '--window-position=0,0',
                    ...(isCI ? ['--disable-gpu'] : []),
                ],
            });

            sharedState.sharedContext = context;

            await use(context);
        },
        { scope: 'test' },
    ],

    extensionId: async ({ context }, use) => {
        if (sharedState.extension.id) {
            await use(sharedState.extension.id);
            return;
        }

        let [background] = context.serviceWorkers();
        if (!background) {
            background = await context.waitForEvent('serviceworker', { timeout: 30000 });
        }

        await new Promise((resolve) => setTimeout(resolve, 1000));

        const extensionId = background.url().split('/')[2];

        sharedState.extension.id = extensionId;

        await use(extensionId);
    },

    extensionUrl: async ({ extensionId }, use) => {
        if (sharedState.extension.url) {
            await use(sharedState.extension.url);
            return;
        }
        const extensionUrl = `chrome-extension://${extensionId}/ui.html`;
        await use(extensionUrl);
    },

    extensionPage: async ({ context, extensionUrl }, use) => {
        const page = await createPage(context, extensionUrl);
        await use(page);
    },

    extensionName: async ({ extensionPage }, use) => {
        if (sharedState.extension.name) {
            await use(sharedState.extension.name);
            return;
        }
        const title = await extensionPage.title();
        await use(title);
    },

    appPage: async ({ context }, use) => {
        const page = await createPage(context, '/');
        await use(page);
    },
});

export const expect = test.expect;
