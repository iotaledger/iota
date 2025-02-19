// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { expect, test } from '@playwright/test';

import { faucet, split_coin } from './utils/localnet';

test('address page', async ({ page }) => {
    const address = await faucet();
    await page.goto(`/address/${address}`);
    await expect(page.getByText('Address')).toBeVisible();
    await expect(page.getByText(address)).toBeVisible();
});

test('owned objects (coins) are displayed', async ({ page }) => {
    const address = await faucet();
    await page.goto(`/address/${address}`);
    await page.waitForSelector('h4:has-text("Owned Objects")');
    await expect(page.getByTestId('ownedcoinlabel')).toContainText('IOTA');
});

test('transactions table is displayed', async ({ page }) => {
    const address = await faucet();
    await split_coin(address);
    await page.goto(`/address/${address}`);
    await page.getByTestId('tx').locator('td').first().waitFor();
});
