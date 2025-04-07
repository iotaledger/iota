// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { test, expect } from './fixtures';
import { importWallet } from './utils';
import 'dotenv/config';

test.describe('Wallet Connection', () => {
    test.beforeEach(async ({ page, extensionUrl }) => {
        // Navigate to the wallet dashboard
        await page.goto('/');
        await page.waitForSelector('.welcome-page');

        // Import a wallet in the extension
        const mnemonic = process.env.TEST_WALLET_MNEMONIC || '';
        await page.goto(extensionUrl);
        await importWallet(page, extensionUrl, mnemonic);

        // Go back to dashboard
        await page.goto('/');
        await page.waitForSelector('.welcome-page');
    });

    test('should connect to wallet extension', async ({ context, page }) => {
        const connectButton = page.getByRole('button', { name: 'Connect' });
        await connectButton.click();

        // Select the extension wallet option
        await page.getByText('IOTA Wallet (DEV)', { exact: true }).click();

        // The extension should appear in a popup, need to handle that
        const approveWalletConnectPage = context.waitForEvent('page');

        // Handle the connection approval in the wallet extension popup
        const walletApprovePage = await approveWalletConnectPage;
        await walletApprovePage.getByText('Continue', { exact: true }).click();
        await walletApprovePage.getByRole('button', { name: 'Connect' }).click();

        // Switch back to main page
        await page.bringToFront();

        // Verify connection was successful on dashboard
        await expect(page.getByTestId('sidebar')).toBeVisible();
    });
});
