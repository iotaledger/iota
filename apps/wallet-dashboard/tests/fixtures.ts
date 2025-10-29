// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/* eslint-disable no-empty-pattern */

import path from 'path';
import os from 'os';
import { test as base, chromium, Page, type BrowserContext } from '@playwright/test';
import { createWallet } from './utils';

const EXTENSION_PATH = path.join(__dirname, '../../wallet/dist');

const DEFAULT_SHARED_STATE = { extension: {}, wallet: {} };

interface SharedState {
    sharedContext?: BrowserContext;
    extension: {
        url?: string;
        name?: string;
    };
    wallet: {
        address?: string;
        mnemonic?: string;
    };
}

let sharedState: SharedState = { ...DEFAULT_SHARED_STATE };

export const test = base.extend<{
    sharedState: SharedState;
    context: BrowserContext;
    pageWithFreshWallet: Page;
    extensionUrl: string;
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

            const userDataDir = path.join(os.tmpdir(), `playwright-${Date.now()}`);

            const launchOptions: Parameters<typeof chromium.launchPersistentContext>[1] = {
                headless: isCI,
                viewport: { width: 720, height: 720 },
                args: [
                    `--disable-extensions-except=${EXTENSION_PATH}`,
                    `--load-extension=${EXTENSION_PATH}`,
                    '--window-position=0,0',
                    '--no-sandbox',
                    '--disable-dev-shm-usage',
                ],
            };

            // Only use chromium channel in CI for headless extension support (Playwright v1.49+)
            if (isCI) {
                launchOptions.channel = 'chromium';
            }

            const context = await chromium.launchPersistentContext(userDataDir, launchOptions);

            sharedState.sharedContext = context;

            await use(context);
        },
        { scope: 'test' },
    ],

    extensionUrl: async ({ context }, use) => {
        if (sharedState.extension.url) {
            await use(sharedState.extension.url);
            return;
        }

        let [background] = context.serviceWorkers();

        // If no service worker is available yet, poll for it instead of waitForEvent
        // This avoids the issue where waitForEvent gets stuck in headless CI mode
        if (!background) {
            const maxAttempts = 60;
            const delayMs = 1000;

            for (let i = 0; i < maxAttempts; i++) {
                [background] = context.serviceWorkers();
                if (background) break;
                await new Promise((resolve) => setTimeout(resolve, delayMs));
            }

            if (!background) {
                throw new Error(
                    'Extension service worker failed to load after 60 seconds. Make sure the wallet extension is built correctly.',
                );
            }
        }

        const extensionId = background.url().split('/')[2];
        const extensionUrl = `chrome-extension://${extensionId}/ui.html`;

        sharedState.extension.url = extensionUrl;

        await use(extensionUrl);
    },

    extensionName: async ({ context, extensionUrl }, use) => {
        // Check if we already have the extension name cached
        if (sharedState.extension.name) {
            await use(sharedState.extension.name);
            return;
        }

        const extPage = await context.newPage();
        await extPage.goto(extensionUrl);

        const extensionName = await extPage.title();
        sharedState.extension.name = extensionName;

        await extPage.close();
        await use(extensionName);
    },

    pageWithFreshWallet: async ({ context, sharedState, extensionUrl }, use) => {
        const extensionPage = await context.newPage();
        await extensionPage.goto(extensionUrl);

        const walletDetails = await createWallet(extensionPage);

        sharedState.wallet.address = walletDetails.address;
        sharedState.wallet.mnemonic = walletDetails.mnemonic;

        await use(extensionPage);
    },
});

test.afterAll(async () => {
    if (sharedState.sharedContext) {
        await sharedState.sharedContext.close();
        sharedState = { ...DEFAULT_SHARED_STATE };
    }
});

export const expect = test.expect;
