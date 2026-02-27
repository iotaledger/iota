// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { expect, test } from './utils/fixtures';
import { createWallet } from './utils/wallet';
import { SHORT_TIMEOUT } from './constants/timeout.constants';

test('account lock-unlock', async ({ page, extensionUrl }) => {
    await createWallet(page, extensionUrl);
    await page.getByTestId('accounts-manage').click();
    await page.getByTestId('lock-wallet').click();
    // Wait for the unlock modal to appear
    await expect(page.getByPlaceholder('Password')).toBeVisible();
    await page.getByPlaceholder('Password').fill('iotae2etests');
    await page.getByRole('button', { name: /Unlock wallet/i }).click();
    // Wait for the unlock modal to disappear
    await expect(page.getByPlaceholder('Password')).not.toBeVisible();
    // Check that the account is unlocked
    await expect(page.getByTestId('lock-wallet')).toBeVisible();
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
    await expect(page.getByPlaceholder('Password')).toBeVisible();
});
