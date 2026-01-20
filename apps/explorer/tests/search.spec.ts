// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { expect, test, type Page } from '@playwright/test';

import { faucet, split_coin } from './utils/localnet';

async function search(page: Page, text: string, resultLabel?: string) {
    const searchbar = page.getByPlaceholder('Search');
    await searchbar.fill(text);

    let result;

    if (resultLabel === 'checkpoint') {
        result = page
            .getByRole('button')
            .filter({ hasText: /^Checkpoint\s+\d+/i })
            .first();
    } else if (resultLabel === 'epoch') {
        result = page
            .getByRole('button')
            .filter({ hasText: /^Epoch\s+\d+/i })
            .first();
    } else {
        result = page.getByRole('button').filter({ hasText: text }).first();
    }

    await expect(result).toBeVisible();
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
    await search(page, '1', 'checkpoint');
    await expect(page).toHaveURL('/checkpoint/1');
});

test('can search for epoch by sequence number', async ({ page }) => {
    await page.goto('/');
    await search(page, '1', 'epoch');
    await expect(page).toHaveURL(/\/epoch\/\d+$/);
});
