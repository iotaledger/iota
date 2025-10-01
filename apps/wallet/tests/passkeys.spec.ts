// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { LONG_TIMEOUT } from './constants/timeout.constants';
import { expect, test } from './fixtures';
import { receiverAddressMnemonic } from './mocks';
import { addVirtualAuthenticator, createPasskeyWallet } from './utils/auth';
import { generateKeypairFromMnemonic } from './utils/localnet';
import { setPresence, setVerified } from './utils/passkeySigner';

const username = 'IOTA Passkey e2e Test';
const displayName = 'IOTAPasskey';

test('Should register a passkey account type with platform authenticator', async ({
    page,
    extensionUrl,
}) => {
    const { client, authenticatorId } = await createPasskeyWallet(page, extensionUrl, {
        username,
        displayName,
    });

    await expect(page.getByText(displayName)).toBeVisible();

    await client.send('WebAuthn.removeVirtualAuthenticator', { authenticatorId });
    await page.close();
});

test('Should register a passkey account type with cross-platform authenticator', async ({
    page,
    extensionUrl,
}) => {
    const { client, authenticatorId } = await createPasskeyWallet(page, extensionUrl, {
        username,
        displayName,
        isCrossPlatform: true,
    });

    await expect(page.getByText(displayName)).toBeVisible();

    await client.send('WebAuthn.removeVirtualAuthenticator', { authenticatorId });
    await page.close();
});

test('Sends funds to another account', async ({ page, extensionUrl }) => {
    const { client, authenticatorId } = await createPasskeyWallet(page, extensionUrl, {
        username,
        displayName,
    });

    await expect(page.getByText(displayName)).toBeVisible();
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
    await page.close();
});

test('Creates a passkey account, resets the wallet and logs back in', async ({
    page,
    extensionUrl,
}) => {
    const { client, authenticatorId } = await createPasskeyWallet(page, extensionUrl, {
        username,
        displayName,
    });

    await expect(page.getByText(displayName)).toBeVisible();

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

    await page.getByText('Add Profile').click();
    await page.getByText('Passkey', { exact: true }).click();
    await page.getByText('Create New Account').click();
    await expect(page.getByText('Restore Existing Account')).toBeVisible();

    await page.getByTestId('username-input').fill(username);
    await page.getByTestId('display-name-input').fill(displayName);

    await page.getByRole('button', { name: /Restore/ }).click();

    await page.getByTestId('password.input').fill('iotae2etests');
    await page.getByTestId('password.confirmation').fill('iotae2etests');

    await page.getByText('I read and agree').click();
    await page.getByRole('button', { name: /Create Wallet/ }).click();

    await expect(page.getByText(displayName)).toBeVisible();
    await page.getByTestId('receive-coin-button').click();

    const newAddressLocator = page.locator("div[data-testid='receive-address']");
    await expect(newAddressLocator).toBeVisible({ timeout: 10_000 });
    const newAddress = (await newAddressLocator.textContent()) || '';
    expect(newAddress.length).toBeGreaterThan(0);
    expect(newAddress).toBe(address);

    await client.send('WebAuthn.removeVirtualAuthenticator', { authenticatorId });
    await page.close();
});

test('Fails when a different authenticator tries to log in', async ({ page, extensionUrl }) => {
    const { client, authenticatorId } = await createPasskeyWallet(page, extensionUrl, {
        username,
        displayName,
    });

    await expect(page.getByText(displayName)).toBeVisible();

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

    // Create a new authenticator
    const { authenticatorId: secondAuthenticatorId } = await addVirtualAuthenticator(client, {
        automaticPresenceSimulation: true,
    });

    await page.getByText('Add Profile').click();
    await page.getByText('Passkey', { exact: true }).click();
    await page.getByText('Create New Account').click();
    await expect(page.getByText('Restore Existing Account')).toBeVisible();

    await page.getByTestId('username-input').fill(username);
    await page.getByTestId('display-name-input').fill(displayName);

    await page.getByRole('button', { name: /Restore/ }).click();

    await page.getByTestId('password.input').fill('iotae2etests');
    await page.getByTestId('password.confirmation').fill('iotae2etests');

    await page.getByText('I read and agree').click();
    await page.getByRole('button', { name: /Create Wallet/ }).click();

    const errorLocator = page.getByText(
        'Passkey operation failed: The operation either timed out or was not allowed.',
    );
    await expect(errorLocator).toBeVisible();

    await client.send('WebAuthn.removeVirtualAuthenticator', { authenticatorId });
    await client.send('WebAuthn.removeVirtualAuthenticator', {
        authenticatorId: secondAuthenticatorId,
    });
    await page.close();
});
