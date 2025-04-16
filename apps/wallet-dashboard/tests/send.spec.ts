// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { test, expect } from './fixtures';
import { connectWallet, createWallet, getAddressByIndexPath } from './utils';

const AMOUNT_TO_SEND = 10;

test.describe('Send Coins', () => {
    test(`should send ${AMOUNT_TO_SEND} IOTA`, async ({ context, extensionUrl, extensionName }) => {
        const extensionPage = await context.newPage();
        await extensionPage.goto(extensionUrl);

        const walletDetails = await createWallet(extensionPage);

        const dashboardPage = await context.newPage();
        await dashboardPage.goto('/');
        await connectWallet(dashboardPage, context, extensionName);

        await extensionPage.bringToFront();
        const originalBalance = await extensionPage.getByTestId('coin-balance').textContent();
        await extensionPage.getByRole('button', { name: /Request \w+ Tokens/ }).click();
        await expect(extensionPage.getByTestId('coin-balance')).not.toHaveText(
            `${originalBalance}`,
            {
                timeout: 30_000,
            },
        );

        await dashboardPage.bringToFront();

        const sendAddress = getAddressByIndexPath(walletDetails.mnemonic, 1);

        const sendButton = dashboardPage.getByTestId('send-coin-button');
        await sendButton.click({ timeout: 30_000 });

        await dashboardPage.getByLabel('Send Amount').fill(AMOUNT_TO_SEND.toString());
        await dashboardPage.getByLabel('Enter Recipient Address').fill(sendAddress);

        await dashboardPage.getByRole('button', { name: 'Review' }).click({ timeout: 30_000 });

        const walletApprovePagePromise = context.waitForEvent('page');
        await dashboardPage.getByRole('button', { name: 'Send Now' }).click({ timeout: 30_000 });

        const walletApprovePage = await walletApprovePagePromise;
        await walletApprovePage.getByRole('button', { name: 'Approve' }).click();

        await dashboardPage.bringToFront();

        await expect(dashboardPage.getByText('Successfully sent')).toBeVisible({
            timeout: 30_000,
        });
    });
});
