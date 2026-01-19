// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { formatBalance, IOTA_DECIMALS, parseAmount } from '@iota/iota-sdk/utils';
import { MIN_NUMBER_IOTA_TO_STAKE } from '@iota/core/src/constants/staking.constants';
import { test, expect } from './utils/fixtures';
import 'dotenv/config';
import {
    getStakedAmount,
    navigateToDashboardStakePage,
    setupWalletWithFunds,
    splitCoinsTransaction,
    submitAndVerifyStaking,
    submitAndVerifyUnstaking,
} from './utils/staking';
import { LONG_TIMEOUT, SHORT_TIMEOUT } from './constants/timeout.constants';

const STAKE_AMOUNT = 100;

test.describe('Wallet staking', () => {
    test('should allow to stake and unstake funds', async ({
        pageWithFreshWallet,
        context,
        sharedState,
    }) => {
        test.setTimeout(LONG_TIMEOUT);
        const dashboardPage = await setupWalletWithFunds(
            pageWithFreshWallet,
            context,
            sharedState.extension.name,
        );
        await navigateToDashboardStakePage(dashboardPage);

        await dashboardPage.getByLabel('Amount').fill(STAKE_AMOUNT.toString());

        await submitAndVerifyStaking(dashboardPage, context);

        await dashboardPage.reload();
        const stakedAmount = await getStakedAmount(dashboardPage);
        expect(stakedAmount).toEqual(STAKE_AMOUNT.toString());

        await submitAndVerifyUnstaking(dashboardPage, context);
    });

    test.describe('Staking with amount selection methods', () => {
        test('should stake using Max button and then unstake', async ({
            pageWithFreshWallet,
            context,
            sharedState,
        }) => {
            test.setTimeout(LONG_TIMEOUT);
            const dashboardPage = await setupWalletWithFunds(
                pageWithFreshWallet,
                context,
                sharedState.extension.name,
            );

            await navigateToDashboardStakePage(dashboardPage);

            await dashboardPage.getByRole('button', { name: 'Max' }).click();
            const inputField = dashboardPage.getByLabel('Amount');
            const maxAmountValue = await inputField.inputValue();
            const maxAmount = parseFloat(maxAmountValue);
            const amountInNanos = parseAmount(maxAmount.toString(), IOTA_DECIMALS);

            await submitAndVerifyStaking(dashboardPage, context);

            await dashboardPage.reload();
            const stakedAmount = await getStakedAmount(dashboardPage);
            expect(stakedAmount).toEqual(formatBalance(amountInNanos, IOTA_DECIMALS));

            await submitAndVerifyUnstaking(dashboardPage, context);
        });

        test('should stake using Recommended amount button and then unstake', async ({
            pageWithFreshWallet,
            context,
            sharedState,
        }) => {
            test.setTimeout(LONG_TIMEOUT);
            const dashboardPage = await setupWalletWithFunds(
                pageWithFreshWallet,
                context,
                sharedState.extension.name,
            );

            await navigateToDashboardStakePage(dashboardPage);

            await dashboardPage.getByRole('button', { name: 'Max' }).click();
            await dashboardPage.getByText('Set recommended amount').click();

            const inputField = dashboardPage.getByLabel('Amount');
            const maxAmountValue = await inputField.inputValue();
            const maxAmount = parseFloat(maxAmountValue);
            const amountInNanos = parseAmount(maxAmount.toString(), IOTA_DECIMALS);

            await submitAndVerifyStaking(dashboardPage, context);

            await dashboardPage.reload();
            const stakedAmount = await getStakedAmount(dashboardPage);
            expect(stakedAmount).toEqual(formatBalance(amountInNanos, IOTA_DECIMALS));

            await submitAndVerifyUnstaking(dashboardPage, context);
        });
    });

    test.describe('Edge case staking amounts', () => {
        test('should stake minimum allowed amount and then unstake', async ({
            pageWithFreshWallet,
            context,
            sharedState,
        }) => {
            test.setTimeout(LONG_TIMEOUT);
            const dashboardPage = await setupWalletWithFunds(
                pageWithFreshWallet,
                context,
                sharedState.extension.name,
            );
            await navigateToDashboardStakePage(dashboardPage);
            await dashboardPage.getByLabel('Amount').fill(MIN_NUMBER_IOTA_TO_STAKE.toString());
            await submitAndVerifyStaking(dashboardPage, context);

            await dashboardPage.reload();
            const stakedAmount = await getStakedAmount(dashboardPage);
            expect(stakedAmount).toEqual(MIN_NUMBER_IOTA_TO_STAKE.toString());

            await submitAndVerifyUnstaking(dashboardPage, context);
        });

        test('should stake max amount minus 1 nano and then unstake', async ({
            pageWithFreshWallet,
            context,
            sharedState,
        }) => {
            test.setTimeout(LONG_TIMEOUT);
            const dashboardPage = await setupWalletWithFunds(
                pageWithFreshWallet,
                context,
                sharedState.extension.name,
            );
            await navigateToDashboardStakePage(dashboardPage);

            await dashboardPage.getByRole('button', { name: 'Max' }).click();
            const inputField = dashboardPage.getByLabel('Amount');
            const maxAmountValue = await inputField.inputValue();
            const maxAmount = parseFloat(maxAmountValue);
            const adjustedAmount = maxAmount - 0.0000001;
            const amountInNanos = parseAmount(adjustedAmount.toString(), IOTA_DECIMALS);
            await inputField.fill('');
            await inputField.fill(adjustedAmount.toString());

            await submitAndVerifyStaking(dashboardPage, context);

            await dashboardPage.reload();
            const stakedAmount = await getStakedAmount(dashboardPage);
            expect(stakedAmount).toEqual(formatBalance(amountInNanos, IOTA_DECIMALS));

            await submitAndVerifyUnstaking(dashboardPage, context);
        });

        test('should stake using multiple small-amount coin objects and then unstake', async ({
            pageWithFreshWallet,
            context,
            sharedState,
        }) => {
            test.setTimeout(LONG_TIMEOUT);
            const dashboardPage = await setupWalletWithFunds(
                pageWithFreshWallet,
                context,
                sharedState.extension.name,
            );
            if (!sharedState.wallet.mnemonic) {
                throw new Error('Wallet mnemonic is undefined');
            }
            await splitCoinsTransaction(sharedState.wallet.mnemonic, 99, 10000000000);
            await navigateToDashboardStakePage(dashboardPage);

            await dashboardPage.getByLabel('Amount').fill('499');

            await submitAndVerifyStaking(dashboardPage, context);

            await dashboardPage.reload();
            const stakedAmount = await getStakedAmount(dashboardPage);
            expect(stakedAmount).toEqual('499');

            await submitAndVerifyUnstaking(dashboardPage, context);
        });

        test('should show error message when staking with over 50 small-amount coin objects', async ({
            pageWithFreshWallet,
            context,
            sharedState,
        }) => {
            test.setTimeout(LONG_TIMEOUT);
            const dashboardPage = await setupWalletWithFunds(
                pageWithFreshWallet,
                context,
                sharedState.extension.name,
            );
            if (!sharedState.wallet.mnemonic) {
                throw new Error('Wallet mnemonic is undefined');
            }
            await splitCoinsTransaction(sharedState.wallet.mnemonic, 99, 10000000000);
            await navigateToDashboardStakePage(dashboardPage);

            await dashboardPage.getByLabel('Amount').fill('500');
            await expect(dashboardPage.getByTestId('error-info-box')).toBeVisible({
                timeout: SHORT_TIMEOUT,
            });
        });
    });
});
