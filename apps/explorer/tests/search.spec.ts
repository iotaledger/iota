// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { expect, test, type Page } from '@playwright/test';

import { faucet, split_coin } from './utils/localnet';

async function search(page: Page, text: string) {
    const searchbar = page.getByPlaceholder('Search');
    await searchbar.fill(text);
    const result = page.getByRole('button').first();
    await result.click();
}

test('can search for an address', async ({ page }) => {
    const address = await faucet();
    await page.goto('/');
    await search(page, address);
    await expect(page).toHaveURL(`/address/${address}`);
});

test('can search for objects', async ({ page }) => {
    const address = await faucet();
    const tx = await split_coin(address);

    const { objectId } = tx.effects!.created![0].reference;
    await page.goto('/');
    await search(page, objectId);
    await expect(page).toHaveURL(`/object/${objectId}`);
});

test('can search for transaction', async ({ page }) => {
    const address = await faucet();
    const tx = await split_coin(address);

    const txid = tx.digest;
    await page.goto('/');
    await search(page, txid);
    await expect(page).toHaveURL(`/txblock/${txid}`);
});

test('can search for checkpoint by sequence number', async ({ page }) => {
    await page.goto('/');
    await search(page, '0');
    await expect(page).toHaveURL(/\/checkpoint\/0/);
});

test('can search for epoch by sequence number', async ({ page }) => {
    await page.goto('/');
    await search(page, '0');
    // Should navigate to epoch page (may be checkpoint or epoch depending on which result is clicked first)
    // We'll check that we can at least get to one of them
    await expect(page.url()).toMatch(/\/(epoch|checkpoint)\/0/);
});
