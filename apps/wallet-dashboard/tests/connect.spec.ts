// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { test, expect } from './fixtures';
import { connectWallet, createWallet, importWallet } from './utils';
import 'dotenv/config';

test.describe.serial('Wallet Connection', () => {
    test.beforeAll(async ({ page, extensionUrl, sharedState, context }) => {
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
        await importWallet(page, extensionUrl, sharedState.walletMnemonic);
        await connectWallet(page, context);

        // Verify connection was successful on dashboard
        const displayedFullAddress = await page
            .locator('[data-full-address]')
            .getAttribute('data-full-address');

        expect(displayedFullAddress).toBe(sharedState.walletAddress);

        await context.close();
    });
});
