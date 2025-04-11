// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { BrowserContext } from '@playwright/test';
import { test, expect } from './fixtures';
import { createWallet, importWallet } from './utils';
import 'dotenv/config';

test.describe.serial('Wallet Connection', () => {
    let browser: BrowserContext;

    test.beforeAll(async ({ page, extensionUrl, sharedState, context }) => {
        await page.goto('/');

        const createdWallet = await createWallet(page, extensionUrl);
        browser = context;

        sharedState.walletMnemonic = createdWallet.mnemonic;
        sharedState.walletAddress = createdWallet.address;
    });

    test('should connect to wallet extension', async ({ extensionUrl, page, sharedState, context }) => {
        await importWallet(page, extensionUrl, sharedState.walletMnemonic);
        await page.goto('/');
        await page.waitForSelector('.welcome-page');
        const connectButton = page.getByRole('button', { name: 'Connect' });

        await connectButton.click();
        const approveWalletConnectPage = context.waitForEvent('page');
        await page.getByText('IOTA Wallet', { exact: true }).click();

        const walletApprovePage = await approveWalletConnectPage;
        await walletApprovePage.bringToFront();
        await walletApprovePage.getByRole('button', { name: 'Continue' }).click();
        await walletApprovePage.getByRole('button', { name: 'Connect' }).click();

        // Switch back to main page
        await page.bringToFront();

        // Verify connection was successful on dashboard
        const displayedFullAddress = await page
            .locator('[data-full-address]')
            .getAttribute('data-full-address');

        expect(displayedFullAddress).toBe(sharedState.walletAddress);
    });
});
