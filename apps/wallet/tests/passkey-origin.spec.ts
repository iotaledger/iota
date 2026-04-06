// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { expect, test } from './utils/fixtures';
import { createPasskeyWallet, TESTS_PASSWORD } from './utils/wallet';

const username = 'Passkeys';
const EXPECTED_RP_ID = 'iota.org';

test(`Passkey origin should be ${EXPECTED_RP_ID} and not other values`, async ({
    page,
    extensionUrl,
}) => {
    let capturedRpId: string | undefined;

    const { client, authenticatorId } = await createPasskeyWallet(page, extensionUrl, {
        username,
        isCrossPlatform: false,
    });

    client.on('WebAuthn.credentialAdded', (params) => {
        capturedRpId = params.credential.rpId;
    });

    const { credentials } = await client.send('WebAuthn.getCredentials', {
        authenticatorId,
    });

    expect(credentials.length).toBeGreaterThan(0);

    const rpId = credentials[0].rpId;

    expect(rpId).toBeDefined();
    expect(rpId).toBe(EXPECTED_RP_ID);
    expect(rpId).not.toContain('chrome-extension://');

    if (capturedRpId) {
        expect(capturedRpId).toBe(EXPECTED_RP_ID);
    }

    await client.send('WebAuthn.removeVirtualAuthenticator', { authenticatorId });
    await client.send('WebAuthn.disable');
    await page.close();
});

test(`Passkey restoration should use ${EXPECTED_RP_ID} origin`, async ({ page, extensionUrl }) => {
    const { client, authenticatorId } = await createPasskeyWallet(page, extensionUrl, {
        username,
        isCrossPlatform: false,
    });

    const { credentials: createdCredentials } = await client.send('WebAuthn.getCredentials', {
        authenticatorId,
    });

    expect(createdCredentials.length).toBeGreaterThan(0);
    expect(createdCredentials[0].rpId).toBe(EXPECTED_RP_ID);

    await page.getByTestId('wallet-settings-button').click();
    await page.getByText('Reset').click();
    await page.getByPlaceholder('Password').fill(TESTS_PASSWORD);
    await page.getByRole('button', { name: 'Verify' }).click();
    await page.getByRole('button', { name: 'Reset' }).click();

    await page.getByRole('button', { name: /Get Started/ }).click();
    await page.getByText('Add existing wallet').click();
    await page.getByText('Passkey', { exact: true }).click();
    await page.getByRole('button', { name: /Continue/ }).click();

    await page.waitForTimeout(1000);

    const { credentials: restoredCredentials } = await client.send('WebAuthn.getCredentials', {
        authenticatorId,
    });

    expect(restoredCredentials.length).toBeGreaterThan(0);
    expect(restoredCredentials[0].rpId).toBe(EXPECTED_RP_ID);

    await client.send('WebAuthn.removeVirtualAuthenticator', { authenticatorId });
    await client.send('WebAuthn.disable');
    await page.close();
});
