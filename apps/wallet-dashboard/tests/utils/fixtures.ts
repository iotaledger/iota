// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/* eslint-disable no-empty-pattern */

import path from 'path';
import { test as base, chromium, Page, type BrowserContext } from '@playwright/test';
import { createWallet } from './wallet';

const EXTENSION_PATH = path.join(__dirname, '../../../wallet/dist');

const DEFAULT_SHARED_STATE = { extension: { url: '', name: '' }, wallet: {} };

const nonPersistentContexts = new Set<BrowserContext>();

export interface SharedState {
    extension: {
        url: string;
        name: string;
    };
    wallet: {
        address?: string;
        mnemonic?: string;
    };
}

interface ContextOptions {
    persistent?: boolean;
}

async function getExtensionUrl(context: BrowserContext): Promise<string> {
    let [background] = context.serviceWorkers();
    if (!background) {
        background = await context.waitForEvent('serviceworker');
    }

    const extensionId = background.url().split('/')[2];
    return `chrome-extension://${extensionId}/ui.html`;
}

async function getExtensionName(context: BrowserContext, extensionUrl: string): Promise<string> {
    const extPage = await context.newPage();
    await extPage.goto(extensionUrl);

    const extensionName = await extPage.title();
    await extPage.close();

    return extensionName;
}

async function setupFreshWallet(context: BrowserContext, sharedState: SharedState): Promise<Page> {
    const extensionUrl = await getExtensionUrl(context);
    const extensionName = await getExtensionName(context, extensionUrl);
    sharedState.extension.url = extensionUrl;
    sharedState.extension.name = extensionName;

    const extensionPage = await context.newPage();
    await extensionPage.goto(extensionUrl);

    const walletDetails = await createWallet(extensionPage);

    sharedState.wallet.address = walletDetails.address;
    sharedState.wallet.mnemonic = walletDetails.mnemonic;
    return extensionPage;
}

export const test = base.extend<{
    sharedState: SharedState;
    createContext: (options?: ContextOptions) => Promise<BrowserContext>;
    context: BrowserContext;
    persistentContext: BrowserContext;
    pageWithFreshWallet: Page;
    pageWithFreshWalletPersistent: Page;
}>({
    sharedState: async ({}, use) => {
        const state: SharedState = DEFAULT_SHARED_STATE;
        await use(state);
    },

    createContext: async ({}, use) => {
        const contextFactory = async (options: ContextOptions = {}) => {
            const { persistent = false } = options;

            const isCI = !!process.env.CI;
            const context = await chromium.launchPersistentContext('', {
                headless: isCI,
                viewport: { width: 720, height: 720 },
                args: [
                    `--disable-extensions-except=${EXTENSION_PATH}`,
                    `--load-extension=${EXTENSION_PATH}`,
                    '--user-agent=Playwright',
                    '--window-position=0,0',
                    ...(isCI ? ['--headless=new', '--disable-gpu'] : []),
                ],
            });

            // Track non-persistent contexts for automatic cleanup
            if (!persistent) {
                nonPersistentContexts.add(context);
            }

            return context;
        };

        await use(contextFactory);

        // Clean up non-persistent contexts
        for (const context of nonPersistentContexts) {
            try {
                await context.close().catch((e) => console.error('Error closing context:', e));
            } catch (e) {
                console.error('Error during context cleanup:', e);
            }
        }
        nonPersistentContexts.clear();
    },

    context: async ({ createContext }, use) => {
        const context = await createContext();
        await use(context);
    },
    persistentContext: async ({ createContext }, use) => {
        const context = await createContext({ persistent: true });
        await use(context);
    },
    pageWithFreshWallet: async ({ context, sharedState }, use) => {
        await use(await setupFreshWallet(context, sharedState));
    },
    pageWithFreshWalletPersistent: async ({ persistentContext, sharedState }, use) => {
        await use(await setupFreshWallet(persistentContext, sharedState));
    },
});

export const expect = test.expect;
