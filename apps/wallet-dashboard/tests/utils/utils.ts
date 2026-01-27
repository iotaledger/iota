// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Page } from '@playwright/test';
import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';
import { expect } from './fixtures';

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
