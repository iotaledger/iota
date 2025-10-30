// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { LONG_TIMEOUT } from './constants/timeout.constants';
import { expect, test } from './fixtures';
import { receiverAddressMnemonic } from './mocks';
import { addVirtualAuthenticator, createPasskeyWallet, restorePasskeyAccount } from './utils/auth';
import { generateKeypairFromMnemonic } from './utils/localnet';
import { setPresence, setVerified } from './utils/passkeySigner';

const username = 'IOTAPasskey';

test('Should register a passkey account type with platform authenticator', async ({
    page,
    extensionUrl,
}) => {
    const { client, authenticatorId } = await createPasskeyWallet(page, extensionUrl, {
        username,
    });

    await client.send('WebAuthn.removeVirtualAuthenticator', { authenticatorId });
    await client.send('WebAuthn.disable');
    await page.close();
});

test('Should register a passkey account type with cross-platform authenticator', async ({
    page,
    extensionUrl,
}) => {
    const { client, authenticatorId } = await createPasskeyWallet(page, extensionUrl, {
        username,
        isCrossPlatform: true,
    });

    await client.send('WebAuthn.removeVirtualAuthenticator', { authenticatorId });
    await client.send('WebAuthn.disable');
    await page.close();
});

test('Sends funds to another account', async ({ page, extensionUrl }) => {
    const { client, authenticatorId } = await createPasskeyWallet(page, extensionUrl, {
        username,
    });

    const receivingKeypair = await generateKeypairFromMnemonic(receiverAddressMnemonic.join(' '));
    const receivingAddress = receivingKeypair.getPublicKey().toIotaAddress();

    await expect(page.getByTestId('coin-balance')).toHaveText('0');

    await page.getByText(/Request localnet tokens/i).click();

    const balanceLocator = page.getByTestId('coin-balance');
    await expect(balanceLocator).not.toHaveText('0', { timeout: LONG_TIMEOUT });

    await page.getByTestId('send-coin-button').click();

    await page.getByRole('button', { name: 'Max' }).click();
    await page.getByPlaceholder('Enter Address').fill(receivingAddress);

    await page.getByText('Review').click();
    await page.getByRole('button', { name: /Send Now/ }).click();

    await expect(page.getByText('Successfully sent')).toBeVisible();

    await client.send('WebAuthn.removeVirtualAuthenticator', { authenticatorId });
    await client.send('WebAuthn.disable');
    await page.close();
});

test('Creates a passkey account, resets the wallet and logs back in', async ({
    page,
    extensionUrl,
}) => {
    const { client, authenticatorId } = await createPasskeyWallet(page, extensionUrl, {
        username,
    });

    await page.getByTestId('receive-coin-button').click();

    const addressLocator = page.locator("div[data-testid='receive-address']");
    await expect(addressLocator).toBeVisible({ timeout: 10_000 });
    const address = (await addressLocator.textContent()) || '';
    expect(address.length).toBeGreaterThan(0);

    await page.getByTestId('close-icon').click();
    await page.getByTestId('wallet-settings-button').click();

    await page.getByText('Reset').click();
    await page.getByRole('button', { name: 'Reset' }).click();

    await expect(page.getByText('IOTA Wallet')).toBeVisible();

    await restorePasskeyAccount(page, username);

    await expect(page.getByText(username)).toBeVisible();
    await page.getByTestId('receive-coin-button').click();

    const newAddressLocator = page.locator("div[data-testid='receive-address']");
    await expect(newAddressLocator).toBeVisible({ timeout: 10_000 });
    const newAddress = (await newAddressLocator.textContent()) || '';
    expect(newAddress.length).toBeGreaterThan(0);
    expect(newAddress).toBe(address);

    await client.send('WebAuthn.removeVirtualAuthenticator', { authenticatorId });
    await client.send('WebAuthn.disable');
    await page.close();
});

test('Fails when a different authenticator tries to log in', async ({ page, extensionUrl }) => {
    const { client, authenticatorId } = await createPasskeyWallet(page, extensionUrl, {
        username,
    });

    await page.getByTestId('receive-coin-button').click();

    const addressLocator = page.locator("div[data-testid='receive-address']");
    await expect(addressLocator).toBeVisible({ timeout: 10_000 });
    const address = (await addressLocator.textContent()) || '';
    expect(address.length).toBeGreaterThan(0);

    await page.getByTestId('close-icon').click();
    await page.getByTestId('wallet-settings-button').click();

    await page.getByText('Reset').click();
    await page.getByRole('button', { name: 'Reset' }).click(); // Dialog confirmation

    await expect(page.getByText('IOTA Wallet')).toBeVisible();

    await setPresence(client, authenticatorId, false);
    await setVerified(client, authenticatorId, false);

    // Remove the authenticator before creating a new one (Chrome only supports one virtual authenticator simultaneously)
    await client.send('WebAuthn.removeVirtualAuthenticator', { authenticatorId });

    // Create a new authenticator
    const { authenticatorId: secondAuthenticatorId } = await addVirtualAuthenticator(client, {
        automaticPresenceSimulation: true,
    });

    await restorePasskeyAccount(page, username);

    const errorLocator = page.getByText(
        'Passkey operation failed: The operation either timed out or was not allowed.',
    );
    await expect(errorLocator).toBeVisible();

    await client.send('WebAuthn.removeVirtualAuthenticator', {
        authenticatorId: secondAuthenticatorId,
    });
    await client.send('WebAuthn.disable');
    await page.close();
});
