// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { test, expect } from './fixtures';
import { connectWallet, createWallet } from './utils';
import 'dotenv/config';

test.describe('Wallet Connection', () => {
    test.beforeEach(async ({ page, extensionUrl, sharedState }) => {
        // Navigate to the wallet
        await page.goto(extensionUrl, { waitUntil: 'load' });

        // Create a wallet in the extension
        const cratedWallet = await createWallet(page);

        sharedState.walletMnemonic = cratedWallet.mnemonic || '';
        sharedState.walletAddress = cratedWallet.address || '';

        // Go to dashboard
        await page.goto('/');
        await page.waitForSelector('.welcome-page');
    });

    test('should connect to wallet extension', async ({
        context,
        page,
        sharedState,
        extensionName,
    }) => {
        await connectWallet(page, context, extensionName);

        // Verify connection was successful on dashboard
        await page.waitForSelector('[data-testid="sidebar"]');
        await expect(page.getByTestId('sidebar')).toBeVisible();

        const displayedFullAddress = await page
            .locator('[data-full-address]')
            .getAttribute('data-full-address');

        expect(displayedFullAddress).toBe(sharedState.walletAddress);
    });

    test('should return to main screen when disconnecting from wallet', async ({
        context,
        page,
        extensionUrl,
        extensionName,
    }) => {
        await connectWallet(page, context, extensionName);

        await page.locator('[data-full-address]').waitFor({ state: 'visible' });

        // Disconnect from the wallet
        const extensionPage = await context.newPage();
        await extensionPage.goto(`${extensionUrl}#/apps/connected`);

        await extensionPage.getByText('localhost').first().click();

        await extensionPage.getByRole('button', { name: 'Disconnect' }).click();

        await page.bringToFront();

        await expect(
            page.getByText('Connecting you to the decentralized web and IOTA network'),
        ).toBeVisible({ timeout: 10000 });

        await expect(page.getByRole('button', { name: 'Connect' })).toBeVisible();
    });
});
