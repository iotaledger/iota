// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Page } from '@playwright/test';

export const PASSWORD = 'iota';

export async function createWallet(page: Page, extensionUrl: string) {
    await page.goto(extensionUrl);
    await page.getByRole('button', { name: /Add Profile/ }).click();
    await page.getByText('Create New').click();
    await page.getByTestId('password.input').fill('iotae2etests');
    await page.getByTestId('password.confirmation').fill('iotae2etests');
    await page.getByText('I read and agree').click();
    await page.getByRole('button', { name: /Create Wallet/ }).click();
    await page.getByText('I saved my mnemonic').click();
    await page.getByRole('button', { name: /Open Wallet/ }).click();
}

export async function importWallet(page: Page, extensionUrl: string, mnemonic: string | string[]) {
    await page.goto(extensionUrl);
    await page.getByRole('link', { name: /More Options/ }).click();
    await page.getByRole('link', { name: /Import Passphrase/ }).click();
    await page
        .getByPlaceholder('Password')
        .first()
        .type(typeof mnemonic === 'string' ? mnemonic : mnemonic.join(' '));
    await page.getByRole('button', { name: /Add Account/ }).click();
    await page.getByLabel('Create Account Password').fill(PASSWORD);
    await page.getByLabel('Confirm Account Password').fill(PASSWORD);
    await page.getByLabel('I read and agreed to the').click();
    await page.getByRole('button', { name: /Create Wallet/ }).click();
}
