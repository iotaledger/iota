// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { expect, test } from './fixtures';
import { createWallet, importWallet } from './utils/auth';
import { generateKeypairFromMnemonic, requestIotaFromFaucet } from './utils/localnet';

const receivedAddressMnemonic = [
    'beef',
    'beef',
    'beef',
    'beef',
    'beef',
    'beef',
    'beef',
    'beef',
    'beef',
    'beef',
    'beef',
    'beef',
];

const currentWalletMnemonic = [
    'intact',
    'drift',
    'gospel',
    'soft',
    'state',
    'inner',
    'shed',
    'proud',
    'what',
    'box',
    'bean',
    'visa',
];

const COIN_TO_SEND = 20;
const timeout = 30_000;

test('request IOTA from local faucet', async ({ page, extensionUrl }) => {
    test.setTimeout(timeout);
    await createWallet(page, extensionUrl);

    const originalBalance = await page.getByTestId('coin-balance').textContent();
    await page.getByText(/Request localnet tokens/i).click();
    await expect(page.getByTestId('coin-balance')).not.toHaveText(`${originalBalance}`, {
        timeout,
    });
});

test('send 20 IOTA to an address', async ({ page, extensionUrl }) => {
    const receivedKeypair = await generateKeypairFromMnemonic(receivedAddressMnemonic.join(' '));
    const receivedAddress = receivedKeypair.getPublicKey().toIotaAddress();

    const originKeypair = await generateKeypairFromMnemonic(currentWalletMnemonic.join(' '));
    const originAddress = originKeypair.getPublicKey().toIotaAddress();

    await importWallet(page, extensionUrl, currentWalletMnemonic);

    await requestIotaFromFaucet(originAddress);
    await expect(page.getByTestId('coin-balance')).not.toHaveText('0', { timeout });

    const originalBalance = await page.getByTestId('coin-balance').textContent();

    await page.getByTestId('send-coin-button').click();
    await page.getByPlaceholder('0.00').fill(String(COIN_TO_SEND));
    await page.getByPlaceholder('Enter Address').fill(receivedAddress);
    await page.getByText('Review').click();
    await page.getByText('Send Now').click();
    await expect(page.getByTestId('overlay-title')).toHaveText('Transaction', { timeout });

    await page.getByTestId('close-icon').click();
    await page.getByTestId('nav-home').click();
    await expect(page.getByTestId('coin-balance')).not.toHaveText(`${originalBalance}`, {
        timeout,
    });
});

test('check balance changes in Activity', async ({ page, extensionUrl }) => {
    const originKeypair = await generateKeypairFromMnemonic(currentWalletMnemonic.join(' '));
    const originAddress = originKeypair.getPublicKey().toIotaAddress();

    await importWallet(page, extensionUrl, currentWalletMnemonic);
    await page.getByTestId('nav-home').click();

    await requestIotaFromFaucet(originAddress);
    await page.getByTestId('nav-activity').click();
    await expect(page.getByTestId('link-to-txn').first()).toBeVisible({ timeout });
    await page.getByTestId('link-to-txn').first().click();
    await expect(page.getByText(`Successfully received`, { exact: false })).toBeVisible();
});
