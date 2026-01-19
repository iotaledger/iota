// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { MIN_NUMBER_IOTA_TO_STAKE } from '@iota/core/src/constants/staking.constants';
import { expect, test } from './utils/fixtures';
import {
    navigateToStakePage,
    navigateToUnstakePage,
    setupWalletWithFunds,
    splitCoinsTransaction,
    submitAndVerifyStaking,
    submitAndVerifyUnstaking,
} from './utils/staking';
import { generateKeypair } from './utils/utils';
import { importWallet } from './utils/wallet';
import { LONG_TIMEOUT, SHORT_TIMEOUT } from './constants/timeout.constants';

const STAKE_AMOUNT = 100;

test.describe('Staking functionality', () => {
    test.describe('Basic staking', () => {
        test('should stake a specific amount and then unstake it', async ({
            page,
            extensionUrl,
        }) => {
            test.setTimeout(LONG_TIMEOUT);
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
    });

    test.describe('Staking with amount selection methods', () => {
        test('should stake using Max button and then unstake', async ({ page, extensionUrl }) => {
            test.setTimeout(LONG_TIMEOUT);
            await setupWalletWithFunds(page, extensionUrl);
            await navigateToStakePage(page);
            await page.getByRole('button', { name: 'Max' }).click();
            await submitAndVerifyStaking(page);
            await navigateToUnstakePage(page);
            await submitAndVerifyUnstaking(page);
        });

        test('should stake using Recommended amount button and then unstake', async ({
            page,
            extensionUrl,
        }) => {
            test.setTimeout(LONG_TIMEOUT);
            await setupWalletWithFunds(page, extensionUrl);
            await navigateToStakePage(page);
            await page.getByRole('button', { name: 'Max' }).click();
            await page.getByText('Set recommended amount').click();
            await submitAndVerifyStaking(page);
            await navigateToUnstakePage(page);
            await submitAndVerifyUnstaking(page);
        });
    });

    test.describe('Edge case staking amounts', () => {
        test('should stake minimum allowed amount and then unstake', async ({
            page,
            extensionUrl,
        }) => {
            test.setTimeout(LONG_TIMEOUT);
            await setupWalletWithFunds(page, extensionUrl);
            await navigateToStakePage(page);
            await page.getByPlaceholder('0 IOTA').fill(MIN_NUMBER_IOTA_TO_STAKE.toString());
            await submitAndVerifyStaking(page);
            await navigateToUnstakePage(page);
            await submitAndVerifyUnstaking(page);
        });

        test('should stake max amount minus 1 nano and then unstake', async ({
            page,
            extensionUrl,
        }) => {
            test.setTimeout(LONG_TIMEOUT);
            await setupWalletWithFunds(page, extensionUrl);
            await navigateToStakePage(page);

            await page.getByRole('button', { name: 'Max' }).click();
            const inputField = page.getByPlaceholder('0 IOTA');
            const maxAmountValue = await inputField.inputValue();
            const maxAmount = parseFloat(maxAmountValue);
            const adjustedAmount = maxAmount - 0.0000001;
            await inputField.fill('');
            await inputField.fill(adjustedAmount.toString());

            await submitAndVerifyStaking(page);
            await navigateToUnstakePage(page);
            await submitAndVerifyUnstaking(page);
        });

        test('should stake using multiple small-amount coin objects and then unstake', async ({
            page,
            extensionUrl,
        }) => {
            test.setTimeout(LONG_TIMEOUT);
            const { mnemonic, keypair } = await generateKeypair();
            await importWallet(page, extensionUrl, mnemonic);
            await page.getByText(/Request localnet tokens/i).click();
            await expect(page.getByTestId('coin-balance')).not.toHaveText('0', {
                timeout: SHORT_TIMEOUT,
            });

            await splitCoinsTransaction(keypair, 99, 10000000000);

            await navigateToStakePage(page);
            await page.getByPlaceholder('0 IOTA').fill('499');
            await submitAndVerifyStaking(page);

            await navigateToUnstakePage(page);
            await submitAndVerifyUnstaking(page);
        });

        test('should show error message when staking with over 50 small-amount coin objects', async ({
            page,
            extensionUrl,
        }) => {
            test.setTimeout(LONG_TIMEOUT);
            const { mnemonic, keypair } = await generateKeypair();
            await importWallet(page, extensionUrl, mnemonic);
            await page.getByText(/Request localnet tokens/i).click();
            await expect(page.getByTestId('coin-balance')).not.toHaveText('0', {
                timeout: SHORT_TIMEOUT,
            });

            await splitCoinsTransaction(keypair, 99, 10000000000);

            await navigateToStakePage(page);
            await page.getByPlaceholder('0 IOTA').fill('500');
            await expect(page.getByTestId('error-info-box')).toBeVisible({
                timeout: SHORT_TIMEOUT,
            });
        });
    });
});
