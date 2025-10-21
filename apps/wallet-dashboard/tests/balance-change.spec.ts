// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Page, BrowserContext } from '@playwright/test';
import { test, expect, SharedState } from './utils/fixtures';
import { requestFaucetTokensOnWalletHome } from './utils/utils';
import { connectWallet } from './utils/wallet';

interface TestContext {
    page: Page;
    context: BrowserContext;
    sharedState: SharedState;
    prevAmount: string | null;
    currentAmount: string | null;
}

test.describe.serial('Balance changes', () => {
    const shared: TestContext = {} as TestContext;

    test.beforeAll(async ({ pageWithFreshWalletPersistent, persistentContext, sharedState }) => {
        shared.page = pageWithFreshWalletPersistent;
        shared.context = persistentContext;
        shared.sharedState = sharedState;
    });

    test(`should request tokens from faucet and see updated balance`, async () => {
        const { page, context, sharedState } = shared;

        if (!sharedState.wallet.mnemonic) {
            throw new Error('Wallet mnemonic not set');
        }

        const dashboardPage = await context.newPage();
        await dashboardPage.goto('/');

        await connectWallet(dashboardPage, context, sharedState.extension.name);

        shared.prevAmount = await dashboardPage.getByTestId('balance-amount').textContent();

        await page.bringToFront();
        await requestFaucetTokensOnWalletHome(page);

        await dashboardPage.bringToFront();
        await dashboardPage.goto('/');

        shared.currentAmount = await dashboardPage.getByTestId('balance-amount').textContent();
        expect(shared.currentAmount).not.toEqual(shared.prevAmount);
        dashboardPage.close();
    });

    test(`should show correct transaction amount in activity section`, async () => {
        const { context, prevAmount, currentAmount } = shared;
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
    test.afterAll(async () => {
        if (shared.context && shared.context.browser()?.isConnected()) {
            await shared.context
                .close()
                .catch((e) => console.error('Error closing persistent context:', e));
        }
    });
});
