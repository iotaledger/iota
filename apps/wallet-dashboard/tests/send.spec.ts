// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { test, expect } from './fixtures';
import { connectWallet, getAddressByIndexPath } from './utils';

const AMOUNT_TO_SEND = 10;

test.describe('Send Coins', () => {
    test(`send ${AMOUNT_TO_SEND} IOTA`, async ({
        context,
        page,
        sharedState,
        extensionName,
        extensionPage,
    }) => {
        await connectWallet(page, context, extensionName);

        await extensionPage.bringToFront();
        const originalBalance = await extensionPage.getByTestId('coin-balance').textContent();
        await extensionPage.getByRole('button', { name: /Request \w+ Tokens/ }).click();
        await expect(extensionPage.getByTestId('coin-balance')).not.toHaveText(
            `${originalBalance}`,
            {
                timeout: 30_000,
            },
        );

        await page.bringToFront();

        const sendAddress = getAddressByIndexPath(sharedState.walletMnemonic, 1);

        const sendButton = page.getByTestId('send-coin-button');
        await sendButton.click({ timeout: 30_000 });

        await page.getByLabel('Send Amount').fill(AMOUNT_TO_SEND.toString());
        await page.getByLabel('Enter Recipient Address').fill(sendAddress);

        await page.getByRole('button', { name: 'Review' }).click({ timeout: 30_000 });

        await page.getByRole('button', { name: 'Send Now' }).click({ timeout: 30_000 });

        const walletApprovePage = await context.waitForEvent('page');
        await walletApprovePage.getByRole('button', { name: 'Approve' }).click();

        await page.bringToFront();

        await expect(page.getByText('Successfully sent')).toBeVisible({
            timeout: 30_000,
        });
    });
});
