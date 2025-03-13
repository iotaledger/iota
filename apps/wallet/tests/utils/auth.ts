// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Page } from '@playwright/test';
import { SHORT_TIMEOUT } from '../constants/timeout.constants';

export const PASSWORD = 'iota';

export async function createWallet(page: Page, extensionUrl: string) {
    await page.goto(extensionUrl, { waitUntil: 'commit' });
    await page.getByRole('button', { name: /Add Profile/ }).click({ timeout: SHORT_TIMEOUT });
    await page.getByText('Create New').click();
    await page.getByTestId('password.input').fill('iotae2etests');
    await page.getByTestId('password.confirmation').fill('iotae2etests');
    await page.getByText('I read and agree').click();
    await page.getByRole('button', { name: /Create Wallet/ }).click();
    await page.getByText('I saved my mnemonic').click();
    await page.getByRole('button', { name: /Open Wallet/ }).click();
}

export async function importWallet(page: Page, extensionUrl: string, mnemonic: string | string[]) {
    await page.goto(extensionUrl, { waitUntil: 'commit' });
    await page.getByRole('button', { name: /Add Profile/ }).click({ timeout: SHORT_TIMEOUT });
    await page.getByText('Mnemonic', { exact: true }).click();
    await page
        .getByPlaceholder('Word')
        .first()
        .fill(typeof mnemonic === 'string' ? mnemonic : mnemonic.join(' '));
    await page.getByText('Add profile').click();
    await page.getByTestId('password.input').fill('iotae2etests');
    await page.getByTestId('password.confirmation').fill('iotae2etests');
    await page.getByText('I read and agree').click();
    await page.getByRole('button', { name: /Create Wallet/ }).click();

    await page.waitForURL(new RegExp(/^(?!.*protect-account).*$/));

    if (await page.getByText('Balance Finder').isVisible()) {
        await page.getByRole('button', { name: /Skip/ }).click();
    }
}
