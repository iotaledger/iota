// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { test, expect } from './fixtures';
import { createWallet } from './utils';
import 'dotenv/config';

test.describe('Wallet Connection', () => {
    test.beforeEach(async ({ page, extensionUrl, sharedState }) => {
        // Navigate to the wallet dashboard
        await page.goto('/');
        await page.waitForSelector('.welcome-page');

        // Import a wallet in the extension
        await page.goto(extensionUrl);
        const cratedWallet = await createWallet(page, extensionUrl);

        sharedState.walletMnemonic = cratedWallet.mnemonic || '';
        sharedState.walletAddress = cratedWallet.address || '';

        // Go back to dashboard
        await page.goto('/');
        await page.waitForSelector('.welcome-page');
    });

    test('should connect to wallet extension', async ({ context, page, sharedState }) => {
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
