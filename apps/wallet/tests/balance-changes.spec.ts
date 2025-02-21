// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { expect, test } from './fixtures';
import { createWallet, importWallet } from './utils/auth';
import { generateKeypairFromMnemonic, requestIotaFromFaucet } from './utils/localnet';
import { SHORT_TIMEOUT, LONG_TIMEOUT } from './constants/timeout.constants';

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

test('request IOTA from local faucet', async ({ page, extensionUrl }) => {
    test.setTimeout(SHORT_TIMEOUT);
    await createWallet(page, extensionUrl);

    const originalBalance = await page.getByTestId('coin-balance').textContent();
    await page.getByText(/Request localnet tokens/i).click();
    await expect(page.getByTestId('coin-balance')).not.toHaveText(`${originalBalance}`, {
        timeout: SHORT_TIMEOUT,
    });
});

test('send 20 IOTA to an address', async ({ page, extensionUrl }) => {
    // Use long timeout in case apps-backend is not available
    test.setTimeout(LONG_TIMEOUT);
    const receivedKeypair = await generateKeypairFromMnemonic(receivedAddressMnemonic.join(' '));
    const receivedAddress = receivedKeypair.getPublicKey().toIotaAddress();

    const originKeypair = await generateKeypairFromMnemonic(currentWalletMnemonic.join(' '));
    const originAddress = originKeypair.getPublicKey().toIotaAddress();

    await importWallet(page, extensionUrl, currentWalletMnemonic);

    await requestIotaFromFaucet(originAddress);
    await expect(page.getByTestId('coin-balance')).not.toHaveText('0', {
        timeout: SHORT_TIMEOUT,
    });

    const originalBalance = await page.getByTestId('coin-balance').textContent();

    await page.waitForSelector('h4:has-text("My Coins")', { timeout: LONG_TIMEOUT });

    await page.getByTestId('send-coin-button').click();
    await page.getByPlaceholder('0.00').fill(String(COIN_TO_SEND));
    await page.getByPlaceholder('Enter Address').fill(receivedAddress);
    await page.getByText('Review').click();
    await page.getByText('Send Now').click();
    await expect(page.getByTestId('overlay-title')).toHaveText('Transaction', {
        timeout: SHORT_TIMEOUT,
    });

    await page.getByTestId('close-icon').click();
    await page.getByTestId('nav-home').click();
    await page.waitForResponse(async (res) => {
        const request = res.request();

        try {
            const postData = request.postDataJSON();
            return postData && postData.method === 'iotax_getBalance';
        } catch {
            return false;
        }
    });
    await expect(page.getByTestId('coin-balance')).not.toHaveText(`${originalBalance}`, {
        timeout: SHORT_TIMEOUT,
    });
});

test('check balance changes in Activity', async ({ page, extensionUrl }) => {
    const originKeypair = await generateKeypairFromMnemonic(currentWalletMnemonic.join(' '));
    const originAddress = originKeypair.getPublicKey().toIotaAddress();

    await importWallet(page, extensionUrl, currentWalletMnemonic);
    await page.getByTestId('nav-home').click();

    await requestIotaFromFaucet(originAddress);
    await page.getByTestId('nav-activity').click();
    await expect(page.getByTestId('link-to-txn').first()).toBeVisible({ timeout: SHORT_TIMEOUT });
    await page.getByTestId('link-to-txn').first().click();
    await expect(page.getByText(`Successfully received`, { exact: false })).toBeVisible();
});
