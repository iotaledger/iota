// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Page, BrowserContext } from '@playwright/test';
import { deriveAddressFromMnemonic } from './utils';
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
    await page.getByRole('button', { name: /Get Started/ }).click({ timeout: 30_000 });
    await page.getByText('Create a new wallet').click();
    await page.getByText('Mnemonic', { exact: true }).click();
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
