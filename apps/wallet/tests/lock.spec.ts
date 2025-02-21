// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { expect, test } from './fixtures';
import { createWallet } from './utils/auth';
import { SHORT_TIMEOUT } from './constants/timeout.constants';

test('account lock-unlock', async ({ page, extensionUrl }) => {
    await createWallet(page, extensionUrl);
    await page.getByTestId('accounts-manage').click();
    await page.getByText('Main').hover();
    await page.getByTestId('account-lock').click();
    await page.getByTestId('account-unlock').click();
    await page.getByPlaceholder('Password').fill('iotae2etests');
    await page.getByRole('button', { name: /Unlock/ }).click();
    await page.getByText('Main').hover();
    await expect(page.getByTestId('account-lock')).toBeVisible();
});

test('wallet auto-lock', async ({ page, extensionUrl }) => {
    test.skip(
        process.env.CI !== 'true',
        'Runs only on CI since it takes at least 1 minute to complete',
    );
    test.setTimeout(100 * 1000);
    await createWallet(page, extensionUrl);
    await page.getByLabel(/Open settings menu/).click();
    await page.getByText(/Auto Lock Profile/).click();
    await page.getByText(/Auto-lock after/i, { exact: false }).click();
    await page.getByRole('button', { name: /Hour/ }).click();
    await page.getByRole('button', { name: /Minute/ }).click();
    await page.getByText('Save').click();
    await expect(page.getByText(/Saved/i)).toBeVisible({ timeout: SHORT_TIMEOUT });
    await page.getByTestId('close-icon').click();
    await page.waitForTimeout(62 * 1000);
    await expect(page.getByText(/Unlock your Account/)).toBeVisible();
});
