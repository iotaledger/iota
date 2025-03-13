// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type Page } from '@playwright/test';
import { expect, test } from './fixtures';
import { createWallet } from './utils/auth';
import { demoDappConnect } from './utils/dappConnect';
import dotenv from 'dotenv';

dotenv.config();

test.beforeEach(async ({ page, extensionUrl }) => {
    await createWallet(page, extensionUrl);
    await page.close();
});

test.describe('Wallet API', () => {
    let demoPage: Page;

    test.beforeEach(async ({ context, demoPageUrl }) => {
        demoPage = await context.newPage();
        await demoDappConnect(demoPage, demoPageUrl, context);
    });
    test('signing message works', async ({ context }) => {
        const newPage = context.waitForEvent('page');
        await demoPage.getByRole('button', { name: 'Sign message' }).click();
        const walletPage = await newPage;
        await walletPage
            .getByRole('button', {
                name: 'Sign',
            })
            .click();
        await expect(demoPage.getByText('Error')).toHaveCount(0);
    });
});
