// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { test, expect } from './fixtures';
import { connectWallet, createWallet } from './utils';
import 'dotenv/config';

test.describe.serial('Wallet Connection', () => {
    let walletAddress: string;

    test.beforeAll(async ({ context, sharedState, extensionUrl, extensionName }) => {
        const page = await context.newPage();
        await page.goto(extensionUrl);

        const cratedWallet = await createWallet(page);

        walletAddress = cratedWallet.address || '';
        sharedState.context = context;
        sharedState.extensionUrl = extensionUrl;
        sharedState.extensionName = extensionName;
    });

    test('should connect to wallet extension', async ({ page, sharedState }) => {
        const { context, extensionName } = sharedState;

        if (!context || !extensionName) {
            throw new Error('Context is not defined');
        }

        await page.goto('/', { waitUntil: 'networkidle' });
        await connectWallet(page, context, extensionName);

        // Verify connection was successful on dashboard
        await expect(page.getByText('Start Staking')).toBeVisible({ timeout: 30_000 });
        const truncatedWalletAddress = walletAddress.slice(0, 6) + '…' + walletAddress.slice(-4);
        await expect(page.getByText(truncatedWalletAddress).first()).toBeVisible({
            timeout: 30_000,
        });

        const displayedFullAddress = await page
            .locator('[data-full-address]')
            .getAttribute('data-full-address');

        expect(displayedFullAddress).toBe(walletAddress);
    });

    test('should return to main screen when disconnecting from wallet', async ({
        page,
        sharedState,
    }) => {
        const { context, extensionUrl, extensionName } = sharedState;

        if (!context || !extensionUrl || !extensionName) {
            throw new Error('Context is not defined');
        }

        await page.goto('/');
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
