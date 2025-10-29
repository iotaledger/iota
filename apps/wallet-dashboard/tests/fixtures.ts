// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/* eslint-disable no-empty-pattern */

import path from 'path';
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

            const context = await chromium.launchPersistentContext('', {
                headless: isCI,
                viewport: { width: 720, height: 720 },
                args: [
                    `--disable-extensions-except=${EXTENSION_PATH}`,
                    `--load-extension=${EXTENSION_PATH}`,
                    '--user-agent=Playwright',
                    '--window-position=0,0',
                    '--no-sandbox',
                    '--disable-dev-shm-usage',
                    ...(isCI ? ['--headless=new'] : []),
                ],
            });

            sharedState.sharedContext = context;

            await use(context);
        },
        { scope: 'test' },
    ],

    extensionUrl: async ({ context }, use) => {
        const isCI = !!process.env.CI;
        let extensionId: string;

        // Try to get extension ID from service worker
        try {
            let [background] = context.serviceWorkers();
            if (!background) {
                background = await context.waitForEvent('serviceworker', {
                    timeout: isCI ? 120000 : 60000,
                });
            }
            extensionId = background.url().split('/')[2];
        } catch (error) {
            // Fallback: Get extension ID from chrome://extensions page
            // This workaround is needed for headless CI environments where service workers may not initialize
            console.warn('Service worker not available, using chrome://extensions fallback');
            const extensionsPage = await context.newPage();
            await extensionsPage.goto('chrome://extensions/');
            await extensionsPage.waitForLoadState('domcontentloaded');

            // Enable developer mode if not already enabled
            const devModeToggle = extensionsPage.locator('#devMode');
            const isChecked = await devModeToggle.evaluate((el: HTMLInputElement) => el.checked);
            if (!isChecked) {
                await devModeToggle.click();
            }

            // Get the extension ID from the extensions page
            const extensionCard = extensionsPage.locator('extensions-item').first();
            extensionId = await extensionCard.evaluate((el: HTMLElement) => el.id);
            await extensionsPage.close();

            if (!extensionId) {
                throw new Error('Could not find extension ID from chrome://extensions');
            }
        }

        const extensionUrl = `chrome-extension://${extensionId}/ui.html`;

        // Wait for extension to be fully loaded and accessible
        const testPage = await context.newPage();
        try {
            await testPage.goto(extensionUrl, {
                waitUntil: 'domcontentloaded',
                timeout: 30000,
            });
            // Wait for the page to be interactive
            await testPage.waitForLoadState('networkidle', { timeout: 10000 });
        } finally {
            await testPage.close();
        }

        sharedState.extension.url = extensionUrl;

        await use(extensionUrl);
    },

    extensionName: async ({ context, extensionUrl }, use) => {
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
