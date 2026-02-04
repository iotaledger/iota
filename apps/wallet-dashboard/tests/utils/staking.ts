// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { BrowserContext, Page } from '@playwright/test';
import { expect } from './fixtures';
import { requestFaucetTokensOnWalletHome } from './utils';
import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';
import { getNetwork, IotaClient } from '@iota/iota-sdk/client';
import { Transaction } from '@iota/iota-sdk/transactions';
import { connectWallet } from './wallet';
import { SHORT_TIMEOUT } from '../constants/timeout.constants';

export async function setupWalletWithFunds(
    page: Page,
    context: BrowserContext,
    extensionName: string,
): Promise<Page> {
    await page.bringToFront();
    await requestFaucetTokensOnWalletHome(page);

    const dashboardPage = await context.newPage();
    await dashboardPage.goto('/');
    await connectWallet(dashboardPage, context, extensionName);
    return dashboardPage;
}

export async function navigateToDashboardStakePage(page: Page): Promise<void> {
    await page.getByTestId('sidebar-staking').click();
    // Move mouse to avoid keeping tooltip open
    await page.mouse.move(200, 0);
    // Wait for tooltip to disappear
    await expect(page.getByRole('tooltip', { name: 'Staking' })).not.toBeVisible({
        timeout: SHORT_TIMEOUT,
    });
    await page.getByRole('button', { name: 'Stake' }).click();

    await page.waitForSelector('text=validator-', {
        state: 'visible',
        timeout: SHORT_TIMEOUT,
    });

    await page.getByText('validator-0').click();
    const nextButton = page.getByText('Next');
    await expect(nextButton).toBeVisible();
    await nextButton.click();

    await expect(page.getByText(/IOTA Available/)).toBeVisible({
        timeout: SHORT_TIMEOUT,
    });
}

export async function submitAndVerifyStaking(page: Page, context: BrowserContext): Promise<void> {
    const stakeButton = page.getByTestId('stake-confirm-btn');
    await expect(stakeButton).toBeEnabled({ timeout: SHORT_TIMEOUT });

    const walletApprovePagePromise = context.waitForEvent('page');
    await stakeButton.click();

    const walletApprovePage = await walletApprovePagePromise;
    await walletApprovePage.getByRole('button', { name: 'Approve' }).click();

    await page.bringToFront();

    await expect(page.getByText('Successfully sent')).toBeVisible({
        timeout: SHORT_TIMEOUT,
    });

    await page.getByTestId('close-icon').click();
}

export async function submitAndVerifyUnstaking(
    page: Page,
    context: BrowserContext,
    validatorName: string = 'validator-0',
): Promise<void> {
    await page.getByText(validatorName).click();
    await page.getByText('Unstake').click();

    const walletApprovePagePromise = context.waitForEvent('page');
    await page.getByRole('button', { name: 'Unstake' }).click();
    const walletApprovePage = await walletApprovePagePromise;
    await walletApprovePage.getByRole('button', { name: 'Approve' }).click();

    await page.bringToFront();
    await page.waitForSelector('text=Start Staking', {
        timeout: SHORT_TIMEOUT,
    });
    expect(page.getByRole('button', { name: 'Stake' })).toBeVisible({
        timeout: SHORT_TIMEOUT,
    });
    expect(page.getByText(validatorName)).not.toBeVisible({ timeout: SHORT_TIMEOUT });
}

export async function getStakedAmount(page: Page): Promise<string | null> {
    return page
        .locator('div:has(> span:text("Your stake"))')
        .locator('xpath=../../div/span')
        .first()
        .textContent();
}

export async function splitCoinsTransaction(
    mnemonic: string,
    objectCount: number,
    amountPerObject: number,
): Promise<string> {
    const keypair = Ed25519Keypair.deriveKeypair(mnemonic);
    const network = getNetwork('localnet');
    const client = new IotaClient({
        url: network.url,
    });
    const tx = new Transaction();

    const splitAmounts = new Array(objectCount).fill(amountPerObject);
    const coins = tx.splitCoins(tx.gas, splitAmounts);

    const coinArgs = [...Array(splitAmounts.length).keys()].map((i) => {
        return {
            kind: 'NestedResult',
            NestedResult: [coins[0].NestedResult[0], i] as [number, number],
        };
    });

    const address = keypair.getPublicKey().toIotaAddress();
    tx.transferObjects(coinArgs, tx.pure.address(address));

    const { digest } = await client.signAndExecuteTransaction({
        transaction: tx,
        signer: keypair,
        options: {
            showEffects: true,
            showEvents: true,
            showInput: true,
            showObjectChanges: true,
        },
    });

    await client.waitForTransaction({
        digest,
        timeout: 30000,
    });

    return digest;
}
