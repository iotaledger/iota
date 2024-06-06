// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { expect, test } from '@playwright/test';

import { faucet, split_coin } from './utils/localnet';

test('address page', async ({ page }) => {
    const address = await faucet();
    await page.goto(`/address/${address}`);
    await expect(page.getByRole('heading', { name: address })).toBeVisible();
});

test('owned objects (coins) are displayed', async ({ page }) => {
    const address = await faucet();
    await page.goto(`/address/${address}`);
    await expect(await page.getByTestId('ownedcoinlabel')).toContainText('IOTA');
});

// TODO: rewrite this test after the removal of devnet_nft
// test('owned objects (nfts) are displayed', async ({ page }) => {
//     const address = await faucet();
//     await mint(address);
//     await page.goto(`/address/${address}`);
//     await expect(page.getByTestId('owned-nfts')).toBeVisible();
// });

test('transactions table is displayed', async ({ page }) => {
    const address = await faucet();
    await split_coin(address);
    await page.goto(`/address/${address}`);
    await page.getByTestId('tx').locator('td').first().waitFor();
});
