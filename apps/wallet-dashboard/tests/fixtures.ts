// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import path from 'path';
import { test as base, chromium, type BrowserContext } from '@playwright/test';

// Path to the wallet extension build directory
const EXTENSION_PATH = path.join(__dirname, '../../wallet/dist');

// Define the shared state type
interface SharedState {
    walletAddress?: string;
    walletMnemonic?: string;
}

const sharedState: SharedState = {};

export const test = base.extend<{
    sharedState: SharedState;
    context: BrowserContext;
    extensionUrl: string;
}>({
    sharedState: async ({ context }, use) => {
        await use(sharedState);
    },

    // Override the default context to load with the extension
    context: async ({ baseURL }, use) => {
        const context = await chromium.launchPersistentContext('', {
            headless: false,
            args: [
                `--disable-extensions-except=${EXTENSION_PATH}`,
                `--load-extension=${EXTENSION_PATH}`,
            ],
        });
        await use(context);
    },

    extensionUrl: async ({ context }, use) => {
        let [background] = context.serviceWorkers();
        if (!background) {
            background = await context.waitForEvent('serviceworker');
        }

        const extensionId = background.url().split('/')[2];
        const extensionUrl = `chrome-extension://${extensionId}/ui.html`;

        await use(extensionUrl);
    },
});

export const expect = test.expect;
