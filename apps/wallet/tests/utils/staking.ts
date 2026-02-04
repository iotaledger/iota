// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { expect, type Page } from '@playwright/test';
import { createWallet } from './wallet';
import { SHORT_TIMEOUT } from 'tests/constants/timeout.constants';
import { Transaction } from '@iota/iota-sdk/transactions';
import { type Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';
import { IotaClient } from '@iota/iota-sdk/client';

export async function setupWalletWithFunds(page: Page, extensionUrl: string) {
    await createWallet(page, extensionUrl);
    await page.getByText(/Request localnet tokens/i).click();
    await expect(page.getByTestId('coin-balance')).not.toHaveText('0', { timeout: SHORT_TIMEOUT });
}

export async function navigateToStakePage(page: Page) {
    await page.getByText(/Start Staking/).click();
    await page
        .getByText(/validator-/, { exact: false })
        .first()
        .click();
    await page.getByText(/Next/).click();
    await expect(page.getByText(/IOTA Available/)).toBeVisible({ timeout: SHORT_TIMEOUT });
}

export async function submitAndVerifyStaking(page: Page) {
    await page.getByRole('button', { name: 'Stake' }).click();

    await expect(page.getByTestId('overlay-title')).toHaveText('Transaction', {
        timeout: SHORT_TIMEOUT,
    });
    await expect(page.getByText(/Successfully sent/)).toBeVisible({ timeout: SHORT_TIMEOUT });
    await expect(page.getByTestId('loading-indicator')).not.toBeVisible({
        timeout: SHORT_TIMEOUT,
    });

    await page.getByTestId('close-icon').click();
}

export async function navigateToUnstakePage(page: Page) {
    await expect(page.getByText(`Current stake`)).toBeVisible({
        timeout: SHORT_TIMEOUT,
    });
    await page.getByText(`Current stake`).click();

    await expect(page.getByTestId('staked-card')).toBeVisible({ timeout: SHORT_TIMEOUT });
    await page.getByTestId('staked-card').click();
    await page.getByText('Unstake').click();
}

export async function submitAndVerifyUnstaking(page: Page) {
    await expect(page.getByTestId('overlay-title')).toHaveText('Unstake');
    await retryAction(async () => {
        await page.getByRole('button', { name: 'Unstake' }).click();
        await expect(page.getByText(/Unstake failed/)).not.toBeVisible({ timeout: 1500 });
        await expect(page.getByTestId('loading-indicator')).not.toBeVisible({
            timeout: SHORT_TIMEOUT,
        });
        await expect(page.getByTestId('overlay-title')).toHaveText('Transaction', {
            timeout: 15000,
        });
    });

    await expect(page.getByText(/Successfully sent/)).toBeVisible({ timeout: SHORT_TIMEOUT });
    await expect(page.getByTestId('loading-indicator')).not.toBeVisible({
        timeout: SHORT_TIMEOUT,
    });

    await page.getByTestId('close-icon').click();
    await expect(page.getByText(`Current stake`)).not.toBeVisible({
        timeout: SHORT_TIMEOUT,
    });
}

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

export async function splitCoinsTransaction(
    keypair: Ed25519Keypair,
    objectCount: number,
    amountPerObject: number,
): Promise<string> {
    const client = new IotaClient({
        url: 'http://localhost:9000',
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
