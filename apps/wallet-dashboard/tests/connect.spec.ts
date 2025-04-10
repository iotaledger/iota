// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { test, expect } from './fixtures';
import { createWallet, importWallet } from './utils';
import 'dotenv/config';

test.describe('Wallet Connection', () => {
    test.beforeAll(async ({ page, extensionUrl, sharedState }) => {
        await page.goto('/');

        const createdWallet = await createWallet(page, extensionUrl);

        sharedState.walletMnemonic = createdWallet.mnemonic;
        sharedState.walletAddress = createdWallet.address;
    });

    test('should connect to wallet extension', async ({
        extensionUrl,
        context,
        page,
        sharedState,
    }) => {
        await importWallet(page, extensionUrl, sharedState.walletMnemonic);
        await page.goto('/');
        await page.waitForSelector('.welcome-page');
        const connectButton = page.getByRole('button', { name: 'Connect' });
        await connectButton.click();

        // Select the extension wallet option
        await page.getByText('IOTA Wallet', { exact: true }).click();

        // The extension should appear in a popup, need to handle that
        const approveWalletConnectPage = context.waitForEvent('page');

        // Handle the connection approval in the wallet extension popup
        const walletApprovePage = await approveWalletConnectPage;
        await walletApprovePage.getByText('Continue', { exact: true }).click();
        await walletApprovePage.getByRole('button', { name: 'Connect' }).click();

        // Switch back to main page
        await page.bringToFront();

        // Verify connection was successful on dashboard
        await page.waitForSelector('[data-testid="sidebar"]');
        await expect(page.getByTestId('sidebar')).toBeVisible();

        const displayedFullAddress = await page
            .locator('[data-full-address]')
            .getAttribute('data-full-address');

        expect(displayedFullAddress).toBe(sharedState.walletAddress);
    });
});
