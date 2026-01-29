// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { BrowserContext, Page } from '@playwright/test';
import { test, expect, SharedState } from './utils/fixtures';
import { connectWallet } from './utils/wallet';
import 'dotenv/config';
import { SHORT_TIMEOUT } from './constants/timeout.constants';

interface TestContext {
    page: Page;
    context: BrowserContext;
    sharedState: SharedState;
}

test.describe.serial('Wallet Connection', () => {
    const shared: TestContext = {} as TestContext;

    test.beforeAll(async ({ pageWithFreshWalletPersistent, persistentContext, sharedState }) => {
        shared.page = pageWithFreshWalletPersistent;
        shared.context = persistentContext;
        shared.sharedState = sharedState;
    });

    test('should connect to wallet extension', async () => {
        const { page, context, sharedState } = shared;

        if (!context) {
            throw new Error('Shared context expected!');
        }

        if (!sharedState.wallet.address) {
            throw new Error('Wallet address was not set');
        }

        await page.goto('/', { waitUntil: 'networkidle' });
        await connectWallet(page, context, sharedState.extension.name);

        // Verify connection was successful on dashboard
        expect(page.getByText('My Coins')).toBeVisible({ timeout: SHORT_TIMEOUT });

        const displayedFullAddress = await page
            .locator('[data-full-address]')
            .getAttribute('data-full-address');

        expect(displayedFullAddress).toBe(sharedState.wallet.address);
    });

    test('should return to main screen when disconnecting from wallet', async () => {
        const { page, context, sharedState } = shared;

        if (!context) {
            throw new Error('Shared context expected!');
        }

        await page.goto('/');
        await page.locator('[data-full-address]').waitFor({ state: 'visible' });

        // Disconnect from the wallet
        const extensionPage = await context.newPage();
        await extensionPage.goto(`${sharedState.extension.url}#/apps/connected`);
        await extensionPage.getByText('localhost').first().click();
        await extensionPage.getByRole('button', { name: 'Disconnect' }).click();

        await page.bringToFront();

        await expect(
            page.getByText('Connecting you to the decentralized web and IOTA network'),
        ).toBeVisible({ timeout: SHORT_TIMEOUT });

        await expect(page.getByRole('button', { name: 'Connect' })).toBeVisible();
    });
    test.afterAll(async () => {
        if (shared.context && shared.context.browser()?.isConnected()) {
            await shared.context
                .close()
                .catch((e) => console.error('Error closing persistent context:', e));
        }
    });
});
