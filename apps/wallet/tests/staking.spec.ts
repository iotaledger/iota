// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { expect, test } from './fixtures';
import { createWallet } from './utils/auth';

const TEST_TIMEOUT = 45 * 1000;
const STAKE_AMOUNT = 100;

test('staking', async ({ page, extensionUrl }) => {
    test.setTimeout(4 * TEST_TIMEOUT);

    await createWallet(page, extensionUrl);

    await page.getByText(/Request localnet tokens/i).click();
    await expect(page.getByTestId('coin-balance')).not.toHaveText('0', { timeout: TEST_TIMEOUT });

    await page.getByText(/Start Staking/).click();
    await page
        .getByText(/validator-/, { exact: false })
        .first()
        .click();
    await page.getByText(/Next/).click();
    await page.getByPlaceholder('0 IOTA').fill(STAKE_AMOUNT.toString());
    await page.getByRole('button', { name: 'Stake' }).click();
    await expect(page.getByTestId('overlay-title')).toHaveText('Transaction');

    await page.getByTestId('close-icon').click();

    await expect(page.getByText(`${STAKE_AMOUNT} IOTA`)).toBeVisible({
        timeout: TEST_TIMEOUT,
    });
    await page.getByText(`${STAKE_AMOUNT} IOTA`).click();

    await expect(page.getByTestId('stake-card')).toBeVisible({ timeout: 3 * TEST_TIMEOUT });
    await page.getByTestId('stake-card').click();
    await page.getByText('Unstake').click();
    await page.getByRole('button', { name: 'Unstake' }).click();
    await expect(page.getByTestId('overlay-title')).toHaveText('Transaction');

    await page.getByTestId('close-icon').click();
    await expect(page.getByText(`${STAKE_AMOUNT} IOTA`)).not.toBeVisible({
        timeout: TEST_TIMEOUT,
    });
});
