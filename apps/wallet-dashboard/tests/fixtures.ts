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
    extensionName: string;
}>({
    sharedState: async ({ context }, use) => {
        await use(sharedState);
    },

    // Override the default context to load with the extension
    context: async ({ baseURL }, use) => {
        const isCI = !!process.env.CI;
        const context = await chromium.launchPersistentContext('', {
            headless: isCI,
            args: [
                `--disable-extensions-except=${EXTENSION_PATH}`,
                `--load-extension=${EXTENSION_PATH}`,
                // Ensure userAgent is correctly set in serviceworker
                '--user-agent=Playwright',
                ...(isCI ? ['--headless=new', '--disable-gpu'] : []),
            ],
        });
        await use(context);
        await context.close();
    },

    // Provide the extension URL to tests
    extensionUrl: async ({ context }, use) => {
        // Get the service worker for the extension
        let [background] = context.serviceWorkers();
        if (!background) {
            background = await context.waitForEvent('serviceworker');
        }

        // Extract extension ID from the service worker URL
        const extensionId = background.url().split('/')[2];
        const extensionUrl = `chrome-extension://${extensionId}/ui.html`;

        await use(extensionUrl);
    },
    extensionName: async ({ context, extensionUrl }, use) => {
        const extPage = await context.newPage();
        await extPage.goto(extensionUrl);

        const extensionName = await extPage.title();
        await use(extensionName);
    },
});

export const expect = test.expect;
