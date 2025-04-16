// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/* eslint-disable no-empty-pattern */

import path from 'path';
import { test as base, chromium, type BrowserContext } from '@playwright/test';

// Path to the wallet extension build directory
const EXTENSION_PATH = path.join(__dirname, '../../wallet/dist');

// Define the shared state type
interface SharedState {
    context?: BrowserContext;
    extensionUrl?: string;
    extensionName?: string;
}

const sharedState: SharedState = {};

export const test = base.extend<{
    sharedState: SharedState;
    context: BrowserContext;
    extensionUrl: string;
    extensionName: string;
}>({
    sharedState: async ({}, use) => {
        await use(sharedState);
    },

    context: [
        async ({ sharedState }, use) => {
            const isCI = !!process.env.CI;
            if (sharedState.context) {
                await use(sharedState.context);
                return;
            }

            const context = await chromium.launchPersistentContext('', {
                headless: isCI,
                args: [
                    `--disable-extensions-except=${EXTENSION_PATH}`,
                    `--load-extension=${EXTENSION_PATH}`,
                    '--user-agent=Playwright',
                    ...(isCI ? ['--headless=new', '--disable-gpu'] : []),
                ],
            });

            sharedState.context = context;

            await use(context);
        },
        { scope: 'test' },
    ],

    extensionUrl: async ({}, use) => {
        if (!sharedState.context) {
            throw new Error('Context not available');
        }

        const context = sharedState.context;

        let [background] = context.serviceWorkers();
        if (!background) {
            background = await context.waitForEvent('serviceworker');
        }

        const extensionId = background.url().split('/')[2];
        const extensionUrl = `chrome-extension://${extensionId}/ui.html`;

        sharedState.extensionUrl = extensionUrl;

        await use(extensionUrl);
    },

    extensionName: async ({}, use) => {
        if (!sharedState.context || !sharedState.extensionUrl) {
            throw new Error('Context or extensionUrl not available');
        }

        const extPage = await sharedState.context.newPage();
        await extPage.goto(sharedState.extensionUrl);

        const extensionName = await extPage.title();
        sharedState.extensionName = extensionName;

        await extPage.close();
        await use(extensionName);
    },
});

test.afterAll(async () => {
    if (sharedState.context) {
        await sharedState.context.close();
        sharedState.context = undefined;
    }
});

export const expect = test.expect;
