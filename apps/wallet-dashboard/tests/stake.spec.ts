// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { test, expect } from './fixtures';
import { connectWallet, requestFaucetTokensOnWalletHome } from './utils';
import 'dotenv/config';

test.describe('Wallet staking', () => {
    test.setTimeout(30_000);

    test('should allow to stake and unstake funds', async ({
        context,
        pageWithFreshWallet,
        extensionName,
    }) => {
        await pageWithFreshWallet.bringToFront();
        await requestFaucetTokensOnWalletHome(pageWithFreshWallet);

        const dashboardPage = await context.newPage();
        await dashboardPage.goto('/');
        await connectWallet(dashboardPage, context, extensionName);

        await dashboardPage.getByTestId('sidebar-staking').click();
        // Move mouse to avoid keeping tooltip open
        await dashboardPage.mouse.move(200, 0);
        // Wait for tooltip to disappear
        await expect(dashboardPage.getByRole('tooltip', { name: 'Staking' })).not.toBeVisible({
            timeout: 5_000,
        });
        await dashboardPage.getByRole('button', { name: 'Stake' }).click();

        await dashboardPage.getByText('validator-0').click();
        await dashboardPage.getByText('Next').click();

        await expect(dashboardPage.getByText(/IOTA Available/)).toBeVisible({ timeout: 30_000 });
        await dashboardPage.getByLabel('Amount').fill('10');

        let stakeButton = dashboardPage.getByTestId('stake-confirm-btn');
        await expect(stakeButton).toBeVisible();

        stakeButton = dashboardPage.getByTestId('stake-confirm-btn');
        let walletApprovePagePromise = context.waitForEvent('page');
        await stakeButton.click();

        let walletApprovePage = await walletApprovePagePromise;
        await walletApprovePage.getByRole('button', { name: 'Approve' }).click();

        await dashboardPage.bringToFront();

        await expect(dashboardPage.getByText('Successfully sent')).toBeVisible({
            timeout: 30_000,
        });

        await dashboardPage.getByTestId('close-icon').click();

        await dashboardPage.reload();

        const stakedAmount = await dashboardPage
            .locator('div:has(> span:text("Your stake"))')
            .locator('xpath=../../div/span')
            .first()
            .textContent();
        expect(stakedAmount).toEqual('10');

        // UNSTAKE
        await dashboardPage.getByText('validator-0').click();
        await dashboardPage.getByText('Unstake').click();

        walletApprovePagePromise = context.waitForEvent('page');
        await dashboardPage.getByRole('button', { name: 'Unstake' }).click();
        walletApprovePage = await walletApprovePagePromise;
        await walletApprovePage.getByRole('button', { name: 'Approve' }).click();

        await dashboardPage.bringToFront();

        await dashboardPage.waitForSelector('text=Start Staking', {
            timeout: 30_000,
        });

        expect(dashboardPage.getByRole('button', { name: 'Stake' })).toBeVisible();
        expect(dashboardPage.getByText('validator-0')).not.toBeVisible();
    });
});
