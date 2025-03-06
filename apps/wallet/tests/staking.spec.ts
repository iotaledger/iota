// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { expect, test } from './fixtures';
import { createWallet } from './utils/auth';

const SHORT_TIMEOUT = 30 * 1000;
const STAKE_AMOUNT = 100;

test('staking', async ({ page, extensionUrl }) => {
    test.setTimeout(4 * SHORT_TIMEOUT);

    await createWallet(page, extensionUrl);

    await page.getByText(/Request localnet tokens/i).click();
    await expect(page.getByTestId('coin-balance')).not.toHaveText('0', { timeout: SHORT_TIMEOUT });

    await page.getByText(/Start Staking/).click();
    await page
        .getByText(/validator-/, { exact: false })
        .first()
        .click();
    await page.getByText(/Next/).click();
    await page.getByPlaceholder('0 IOTA').fill(STAKE_AMOUNT.toString());
    await page.getByRole('button', { name: 'Stake' }).click();

    await expect(page.getByTestId('overlay-title')).toHaveText('Transaction', {
        timeout: SHORT_TIMEOUT,
    });
    await expect(page.getByText(/Successfully sent/)).toBeVisible({ timeout: SHORT_TIMEOUT });

    await expect(page.getByTestId('loading-indicator')).not.toBeVisible({
        timeout: SHORT_TIMEOUT,
    });

    await page.getByTestId('close-icon').click();

    await expect(page.getByText(`${STAKE_AMOUNT} IOTA`)).toBeVisible({
        timeout: SHORT_TIMEOUT,
    });
    await page.getByText(`${STAKE_AMOUNT} IOTA`).click();

    await expect(page.getByTestId('staked-card')).toBeVisible({ timeout: SHORT_TIMEOUT });
    await page.getByTestId('staked-card').click();
    await page.getByText('Unstake').click();

    await expect(page.getByTestId('overlay-title')).toHaveText('Unstake');

    await retryAction(async () => {
        // we retry the unstaking action
        await page.getByRole('button', { name: 'Unstake' }).click();
        // until there is no unstake error
        await expect(page.getByText(/Unstake failed/)).not.toBeVisible({ timeout: 1500 });
        // loading of the page is done
        await expect(page.getByTestId('loading-indicator')).not.toBeVisible({
            timeout: SHORT_TIMEOUT,
        });
        // and we land on the next page
        await expect(page.getByTestId('overlay-title')).toHaveText('Transaction', {
            timeout: 15000,
        });
    });

    await expect(page.getByText(/Successfully sent/)).toBeVisible({ timeout: SHORT_TIMEOUT });
    await expect(page.getByTestId('loading-indicator')).not.toBeVisible({
        timeout: SHORT_TIMEOUT,
    });

    await page.getByTestId('close-icon').click();
    await expect(page.getByText(`${STAKE_AMOUNT} IOTA`)).not.toBeVisible({
        timeout: SHORT_TIMEOUT,
    });
});

async function retryAction<T>(action: () => Promise<T>, maxRetries = 3, delay = 2500) {
    for (let attempt = 1; attempt <= maxRetries; attempt++) {
        try {
            await action();
            return;
        } catch (error: unknown) {
            if (attempt < maxRetries) {
                // eslint-disable-next-line no-console
                console.log(`Retrying action in ${delay} ms`);
                await new Promise((resolve) => setTimeout(resolve, delay));
            }
        }
    }

    throw new Error(`Action failed after ${maxRetries} attempts.`);
}
