// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { BrowserContext, Page } from '@playwright/test';

import { expect } from '../fixtures';
import { SHORT_TIMEOUT } from '../constants/timeout.constants';

export async function demoDappConnect(page: Page, demoPageUrl: string, context: BrowserContext) {
    await page.goto(demoPageUrl, { waitUntil: 'commit' });
    const newWalletPage = context.waitForEvent('page');
    await page.getByRole('button', { name: 'Connect' }).click({ timeout: SHORT_TIMEOUT });
    const walletPage = await newWalletPage;
    await walletPage.waitForLoadState();
    await walletPage.getByRole('button', { name: 'Continue' }).click();
    await walletPage.getByRole('button', { name: 'Connect' }).click();
    const accountsList = page.getByTestId('accounts-list');
    const accountListItems = accountsList.getByRole('listitem');
    await expect(accountListItems).toHaveCount(1);
}
