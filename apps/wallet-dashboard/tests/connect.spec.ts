// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { test, expect } from './fixtures';
import { connectWallet, createWallet } from './utils';
import 'dotenv/config';

test.describe.serial('Wallet Connection', () => {
    test.beforeAll(async ({ context, sharedState, extensionUrl }) => {
        const page = await context.newPage();
        await page.goto(extensionUrl);

        const cratedWallet = await createWallet(page);

        sharedState.wallet.address = cratedWallet.address;
    });

    test('should connect to wallet extension', async ({ page, sharedState, extensionName }) => {
        const { sharedContext } = sharedState;

        if (!sharedContext) {
            throw new Error('Context is not defined');
        }

        await page.goto('/', { waitUntil: 'networkidle' });
        await connectWallet(page, sharedContext, extensionName);

        // Verify connection was successful on dashboard
        await page.waitForSelector('[data-testid="sidebar"]');
        await expect(page.getByTestId('sidebar')).toBeVisible();

        const displayedFullAddress = await page
            .locator('[data-full-address]')
            .getAttribute('data-full-address');

        expect(displayedFullAddress).toBe(sharedState.wallet.address);
    });

    test('should return to main screen when disconnecting from wallet', async ({
        page,
        sharedState,
        extensionUrl,
    }) => {
        const { sharedContext } = sharedState;

        if (!sharedContext) {
            throw new Error('Context is not defined');
        }

        await page.goto('/');
        await page.locator('[data-full-address]').waitFor({ state: 'visible' });

        // Disconnect from the wallet
        const extensionPage = await sharedContext.newPage();
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
