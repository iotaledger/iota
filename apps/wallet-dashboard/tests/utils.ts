// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Page, BrowserContext } from '@playwright/test';
import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';
import { expect } from './fixtures';

export async function connectWallet(page: Page, context: BrowserContext, extensionName: string) {
    await page.getByRole('button', { name: 'Connect' }).click();

    const pagePromise = context.waitForEvent('page', { timeout: 20_000 });
    await page.getByText(extensionName, { exact: true }).click();
    const walletApprovePage = await pagePromise;

    await walletApprovePage.waitForLoadState('load');
    await walletApprovePage.bringToFront();

    await walletApprovePage.getByRole('button', { name: 'Continue' }).click();
    await walletApprovePage.getByRole('button', { name: 'Connect' }).click();

    await page.bringToFront();
}

export async function createWallet(page: Page) {
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

export async function requestFaucetTokensOnWalletHome(page: Page) {
    const originalBalance = await page.getByTestId('coin-balance').textContent();
    await page.getByRole('button', { name: /Request \w+ Tokens/ }).click();
    await expect(page.getByTestId('coin-balance')).not.toHaveText(`${originalBalance}`, {
        timeout: 30_000,
    });
}

export function deriveAddressFromMnemonic(mnemonic: string, path?: string) {
    const keypair = Ed25519Keypair.deriveKeypair(mnemonic, path);
    const address = keypair.getPublicKey().toIotaAddress();
    return address;
}

export function getAddressByIndexPath(mnemonic: string, index: number) {
    return deriveAddressFromMnemonic(mnemonic, `m/44'/4218'/0'/0'/${index}'`);
}
