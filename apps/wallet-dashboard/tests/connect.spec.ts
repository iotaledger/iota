// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { test, expect } from './fixtures';
import { connectWallet } from './utils';
import 'dotenv/config';

test.describe('Wallet Connection', () => {
    test('should connect to wallet extension', async ({
        context,
        page,
        sharedState,
        extensionName,
    }) => {
        await connectWallet(page, context, extensionName);

        // Switch back to main page
        await page.bringToFront();

        // Verify connection was successful on dashboard
        await expect(page.getByText('Start Staking')).toBeVisible({ timeout: 30_000 });
        const truncatedWalletAddress =
            sharedState.walletAddress.slice(0, 6) + '…' + sharedState.walletAddress.slice(-4);
        await expect(page.getByText(truncatedWalletAddress).first()).toBeVisible({
            timeout: 30_000,
        });

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

        // Switch back to main page
        await page.bringToFront();

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
