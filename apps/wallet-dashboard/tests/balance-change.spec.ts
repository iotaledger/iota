// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { test, expect } from './fixtures';
import { connectWallet, requestFaucetTokensOnWalletHome } from './utils';

test.describe('Balance changes', () => {
    test(`should request tokens from faucet and see updated balance`, async ({
        context,
        pageWithFreshWallet,
        sharedState,
        extensionName,
    }) => {
        const { wallet } = sharedState;

        if (!wallet.mnemonic) {
            throw new Error('Wallet mnemonic not set');
        }

        const dashboardPage = await context.newPage();
        await dashboardPage.goto('/');

        await connectWallet(dashboardPage, context, extensionName);

        const prevAmount = await dashboardPage.getByTestId('balance-amount').textContent();

        await pageWithFreshWallet.bringToFront();
        await requestFaucetTokensOnWalletHome(pageWithFreshWallet);

        await dashboardPage.bringToFront();
        await dashboardPage.goto('/');

        const currentAmount = await dashboardPage.getByTestId('balance-amount').textContent();
        expect(currentAmount).not.toEqual(prevAmount);
    });
});
