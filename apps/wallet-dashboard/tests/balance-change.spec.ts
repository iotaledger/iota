// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { test, expect } from './fixtures';
import { connectWallet, requestFaucetTokensOnWalletHome } from './utils';

test.describe.serial('Balance changes', () => {
    let prevAmount: string | null;
    let currentAmount: string | null;

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

        prevAmount = await dashboardPage.getByTestId('balance-amount').textContent();

        await pageWithFreshWallet.bringToFront();
        await requestFaucetTokensOnWalletHome(pageWithFreshWallet);

        await dashboardPage.bringToFront();
        await dashboardPage.goto('/');

        currentAmount = await dashboardPage.getByTestId('balance-amount').textContent();
        expect(currentAmount).not.toEqual(prevAmount);
        dashboardPage.close();
    });

    test(`should show correct transaction amount in activity section`, async ({ context }) => {
        test.skip(!prevAmount || !currentAmount, 'No balance change data available');

        const prevAmountValue = parseFloat(prevAmount!.replace(/[^0-9.-]+/g, '') || '0');
        const currentAmountValue = parseFloat(currentAmount!.replace(/[^0-9.-]+/g, '') || '0');
        const balanceChange = currentAmountValue - prevAmountValue;

        const dashboardPage = await context.newPage();
        await dashboardPage.goto('/');

        const transactionTile = dashboardPage
            .getByTestId('home-page-activity-section')
            .getByTestId('transaction-tile')
            .first();
        await transactionTile.waitFor({ state: 'visible' });

        const tileTexts = await transactionTile.allInnerTexts();
        const iotaAmountText = tileTexts.find((text) => text.includes('IOTA'));

        expect(iotaAmountText).toBeTruthy();

        if (!iotaAmountText) {
            throw new Error('No IOTA amount found in transaction tile');
        }

        const match = iotaAmountText.replace(/,/g, '').match(/(\d+(\.\d+)?)\s*IOTA/);
        expect(match).toBeTruthy();

        if (!match) {
            throw new Error('Failed to extract amount from text: ' + iotaAmountText);
        }

        const txAmountValue = parseFloat(match[1]);

        expect(txAmountValue).toBeCloseTo(balanceChange, 2);
        await dashboardPage.close();
    });
});
