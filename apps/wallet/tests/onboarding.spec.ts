// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { expect, test } from './fixtures';
import { createWallet, importWallet } from './utils/auth';
import { generateKeypair } from './utils/localnet';
import * as bip39 from '@scure/bip39';
import { wordlist } from '@scure/bip39/wordlists/english';

test('create new wallet', async ({ page, extensionUrl }) => {
    await createWallet(page, extensionUrl);
    await page.getByTestId('nav-home').click();
    await expect(page.getByTestId('coin-page')).toBeVisible();
});

test('import wallet', async ({ page, extensionUrl }) => {
    const { mnemonic, keypair } = await generateKeypair();
    importWallet(page, extensionUrl, mnemonic);
    await page.getByTestId('nav-home').click();
    await expect(
        page.getByText(keypair.getPublicKey().toIotaAddress().slice(0, 6)).first(),
    ).toBeVisible();
});

test('import wallet with 12 words', async ({ page, extensionUrl }) => {
    const mnemonic = bip39.generateMnemonic(wordlist, 128);
    importWallet(page, extensionUrl, mnemonic);
    await page.getByTestId('nav-home').click();
    await expect(page.getByTestId('coin-page')).toBeVisible();
});

test('import wallet with 24 words', async ({ page, extensionUrl }) => {
    const mnemonic = bip39.generateMnemonic(wordlist, 256);
    importWallet(page, extensionUrl, mnemonic);
    await page.getByTestId('nav-home').click();
    await expect(page.getByTestId('coin-page')).toBeVisible();
});
