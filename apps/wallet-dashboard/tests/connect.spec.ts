// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { test, expect } from './fixtures';
import { createWallet, importWallet } from './utils';
import 'dotenv/config';

test.describe.serial('Wallet Connection', () => {
    test.beforeAll(async ({ page, extensionUrl, sharedState, context }) => {
        await page.goto('/');

        const createdWallet = await createWallet(page, extensionUrl);

        sharedState.walletMnemonic = createdWallet.mnemonic;
        sharedState.walletAddress = createdWallet.address;
    });

    test('should connect to wallet extension', async ({
        extensionUrl,
        page,
        sharedState,
        context,
    }) => {
        test.setTimeout(120000);

        await importWallet(page, extensionUrl, sharedState.walletMnemonic);
        await page.goto('/');
        await page.waitForSelector('.welcome-page');
        const connectButton = page.getByRole('button', { name: 'Connect' });

        const pagePromise = context.waitForEvent('page', { timeout: 60000 });

        await page.waitForTimeout(1000);
        await connectButton.click();
        await page.waitForTimeout(1000);
        await page.getByText('IOTA Wallet', { exact: true }).click();
        await page.waitForTimeout(1000);

        let walletApprovePage;
        try {
            walletApprovePage = await pagePromise;
        } catch (error) {
            await page.screenshot({ path: 'error-waiting-for-page.png' });
            const isContextValid = !context.browser()?.isConnected;
            console.error('Is context still valid?', !isContextValid);
            throw error;
        }
        await walletApprovePage.waitForLoadState('load');
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
