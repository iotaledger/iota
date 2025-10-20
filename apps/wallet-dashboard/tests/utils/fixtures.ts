// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/* eslint-disable no-empty-pattern */

import path from 'path';
import { test as base, chromium, Page, type BrowserContext } from '@playwright/test';
import { createWallet } from './wallet';

const EXTENSION_PATH = path.join(__dirname, '../../../wallet/dist');

const DEFAULT_SHARED_STATE = { extension: {}, wallet: {} };

interface SharedState {
    extension: {
        url?: string;
        name?: string;
    };
    wallet: {
        address?: string;
        mnemonic?: string;
    };
}

export const test = base.extend<{
    sharedState: SharedState;
    context: BrowserContext;
    pageWithFreshWallet: Page;
    extensionUrl: string;
    extensionName: string;
}>({
    sharedState: async ({}, use) => {
        const state: SharedState = DEFAULT_SHARED_STATE;
        await use(state);
    },

    context: async ({}, use) => {
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

        await use(context);
        await context.close();
    },

    extensionUrl: async ({ context, sharedState }, use) => {
        let [background] = context.serviceWorkers();
        if (!background) {
            background = await context.waitForEvent('serviceworker');
        }

        const extensionId = background.url().split('/')[2];
        const extensionUrl = `chrome-extension://${extensionId}/ui.html`;

        sharedState.extension.url = extensionUrl;

        await use(extensionUrl);
    },

    extensionName: async ({ context, extensionUrl, sharedState }, use) => {
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

export const expect = test.expect;
