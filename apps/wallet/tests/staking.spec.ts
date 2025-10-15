// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { MIN_NUMBER_IOTA_TO_STAKE } from '@iota/core/src/constants/staking.constants';
import { expect, test } from './fixtures';
import {
    navigateToStakePage,
    navigateToUnstakePage,
    setupWalletWithFunds,
    submitAndVerifyStaking,
    submitAndVerifyUnstaking,
} from './utils/staking';

const SHORT_TIMEOUT = 30 * 1000;
const STAKE_AMOUNT = 100;

test('staking', async ({ page, extensionUrl }) => {
    test.setTimeout(4 * SHORT_TIMEOUT);

    await setupWalletWithFunds(page, extensionUrl);

    await navigateToStakePage(page);

    await page.getByPlaceholder('0 IOTA').fill(STAKE_AMOUNT.toString());
    await submitAndVerifyStaking(page);

    await navigateToUnstakePage(page);
    await submitAndVerifyUnstaking(page);
    await expect(page.getByText(`${STAKE_AMOUNT} IOTA`)).not.toBeVisible({
        timeout: SHORT_TIMEOUT,
    });
});

test('stake max amount using Max button', async ({ page, extensionUrl }) => {
    test.setTimeout(4 * SHORT_TIMEOUT);

    await setupWalletWithFunds(page, extensionUrl);

    await navigateToStakePage(page);

    await page.getByRole('button', { name: 'Max' }).click();
    await submitAndVerifyStaking(page);

    await navigateToUnstakePage(page);
    await submitAndVerifyUnstaking(page);
});

test('stake max recommended amount', async ({ page, extensionUrl }) => {
    test.setTimeout(4 * SHORT_TIMEOUT);

    await setupWalletWithFunds(page, extensionUrl);

    await navigateToStakePage(page);

    await page.getByRole('button', { name: 'Max' }).click();
    await page.getByText('Set recommended amount').click();
    await submitAndVerifyStaking(page);

    await navigateToUnstakePage(page);
    await submitAndVerifyUnstaking(page);
});

test('stake min amount', async ({ page, extensionUrl }) => {
    test.setTimeout(4 * SHORT_TIMEOUT);

    await setupWalletWithFunds(page, extensionUrl);

    await navigateToStakePage(page);

    await page.getByPlaceholder('0 IOTA').fill(MIN_NUMBER_IOTA_TO_STAKE.toString());
    await submitAndVerifyStaking(page);

    await navigateToUnstakePage(page);
    await submitAndVerifyUnstaking(page);
});

test('stake max amount minus 1 nano', async ({ page, extensionUrl }) => {
    test.setTimeout(4 * SHORT_TIMEOUT);

    await setupWalletWithFunds(page, extensionUrl);

    await navigateToStakePage(page);

    await page.getByRole('button', { name: 'Max' }).click();

    const inputField = page.getByPlaceholder('0 IOTA');
    const maxAmountStr = await inputField.inputValue();

    const maxAmount = parseFloat(maxAmountStr);
    const adjustedAmount = Math.max(0, maxAmount - 0.0000001);

    await inputField.fill('');
    await inputField.fill(adjustedAmount.toString());

    await submitAndVerifyStaking(page);

    await navigateToUnstakePage(page);
    await submitAndVerifyUnstaking(page);
});
