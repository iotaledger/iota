// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Page, BrowserContext } from '@playwright/test';
import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';
import { expect } from './fixtures';

export async function connectWallet(page: Page, context: BrowserContext) {
    await page.goto('/');
    await page.waitForSelector('.welcome-page');
    const connectButton = page.getByRole('button', { name: 'Connect' });

    const pagePromise = context.waitForEvent('page', { timeout: 60000 });
    await connectButton.click();
    await page.getByText('IOTA Wallet', { exact: true }).click();
    const walletApprovePage = await pagePromise;

    await walletApprovePage.waitForLoadState('load');
    await walletApprovePage.bringToFront();

    await walletApprovePage.getByRole('button', { name: 'Continue' }).click();
    await walletApprovePage.getByRole('button', { name: 'Connect' }).click();

    await page.bringToFront();
}

export async function createWallet(page: Page, extensionUrl: string) {
    await page.goto(extensionUrl, { waitUntil: 'commit' });
    await page.getByRole('button', { name: /Add Profile/ }).click({ timeout: 30000 });
    await page.getByText('Create New', { exact: true }).click();
    await page.getByTestId('password.input').fill('iotae2etests');
    await page.getByTestId('password.confirmation').fill('iotae2etests');
    await page.getByText('I read and agree').click();

    await page.getByRole('button', { name: /Create Wallet/ }).click();
    await page.waitForURL(new RegExp(/accounts\/backup/));

    const BOX_TEST_ID = 'mnemonic-display-box';
    const mnemonicBox = page.getByTestId(BOX_TEST_ID);

    await expect(mnemonicBox).toBeVisible();

    await mnemonicBox.getByRole('button').first().click();
    const textarea = mnemonicBox.locator('textarea');
    const mnemonic = await textarea.inputValue();

    const address = deriveAddressFromMnemonic(mnemonic);

    await page.getByText('I saved my mnemonic').click();
    await page.getByRole('button', { name: 'Open Wallet' }).click();

    return {
        mnemonic,
        address,
    };
}

export async function importWallet(page: Page, extensionUrl: string, mnemonic?: string) {
    if (!mnemonic) {
        throw new Error('Mnemonic is required for importing a wallet');
    }
    await page.goto(extensionUrl, { waitUntil: 'commit' });
    await page.getByRole('button', { name: /Add Profile/ }).click({ timeout: 30000 });
    await page.getByText('Mnemonic', { exact: true }).click();

    const mnemonicArray = mnemonic.split(' ');

    const wordInputs = page.locator('input[placeholder="Word"]');
    const inputCount = await wordInputs.count();

    for (let i = 0; i < inputCount; i++) {
        await wordInputs.nth(i).fill(mnemonicArray[i]);
    }

    await page.getByText('Add profile').click();
    await page.getByTestId('password.input').fill('iotae2etests');
    await page.getByTestId('password.confirmation').fill('iotae2etests');
    await page.getByText('I read and agree').click();
    await page.getByRole('button', { name: /Create Wallet/ }).click();
    await page.waitForURL(new RegExp(/^(?!.*accounts).*$/));
}

export function deriveAddressFromMnemonic(mnemonic: string) {
    const keypair = Ed25519Keypair.deriveKeypair(mnemonic);
    const address = keypair.getPublicKey().toIotaAddress();
    return address;
}
