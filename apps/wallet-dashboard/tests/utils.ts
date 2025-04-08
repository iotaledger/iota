// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Page } from '@playwright/test';
import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';

export async function importWallet(page: Page, extensionUrl: string, mnemonic: string) {
    await page.goto(extensionUrl, { waitUntil: 'commit' });
    await page.getByRole('button', { name: /Add Profile/ }).click({ timeout: 30000 });
    await page.getByText('Mnemonic', { exact: true }).click();

    const mnemonicArray = typeof mnemonic === 'string' ? mnemonic.split(' ') : mnemonic;

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
    await page.waitForURL(new RegExp(/^(?!.*protect-account).*$/));
}

export function deriveAddressFromMnemonic(mnemonic: string) {
    const keypair = Ed25519Keypair.deriveKeypair(mnemonic);
    const address = keypair.getPublicKey().toIotaAddress();
    return address;
}
