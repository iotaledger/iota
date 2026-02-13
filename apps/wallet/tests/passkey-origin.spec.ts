// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { expect, test } from './utils/fixtures';
import { SHORT_TIMEOUT } from './constants/timeout.constants';

const username = 'Passkeys';
const EXPECTED_RP_ID = 'iota.org';

test(`Passkey origin should be ${EXPECTED_RP_ID} and not other values`, async ({
    page,
    extensionUrl,
}) => {
    const client = await page.context().newCDPSession(page);
    await client.send('WebAuthn.enable');

    const { authenticatorId } = await client.send('WebAuthn.addVirtualAuthenticator', {
        options: {
            protocol: 'ctap2',
            transport: 'internal',
            hasResidentKey: true,
            hasUserVerification: true,
            isUserVerified: true,
            automaticPresenceSimulation: true,
        },
    });

    await page.goto(extensionUrl, { waitUntil: 'commit' });
    await page.getByRole('button', { name: /Get Started/ }).click({ timeout: SHORT_TIMEOUT });
    await page.getByText('Create a new wallet').click();
    await page.getByText('Passkey', { exact: true }).click();

    await page.getByTestId('username-input').fill(username);
    await page.getByTestId('passkey-radio-platform').click();

    let capturedRpId: string | undefined;

    client.on('WebAuthn.credentialAdded', (params) => {
        capturedRpId = params.credential.rpId;
    });

    await page.getByRole('button', { name: /Continue/ }).click();

    await page.getByTestId('password.input').fill('iotae2etests');
    await page.getByTestId('password.confirmation').fill('iotae2etests');
    await page.getByText('I read and agree').click();
    await page.getByRole('button', { name: /Create Wallet/ }).click();

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
    const client = await page.context().newCDPSession(page);
    await client.send('WebAuthn.enable');

    const { authenticatorId } = await client.send('WebAuthn.addVirtualAuthenticator', {
        options: {
            protocol: 'ctap2',
            transport: 'internal',
            hasResidentKey: true,
            hasUserVerification: true,
            isUserVerified: true,
            automaticPresenceSimulation: true,
        },
    });

    await page.goto(extensionUrl, { waitUntil: 'commit' });
    await page.getByRole('button', { name: /Get Started/ }).click({ timeout: SHORT_TIMEOUT });
    await page.getByText('Create a new wallet').click();
    await page.getByText('Passkey', { exact: true }).click();

    await page.getByTestId('username-input').fill(username);
    await page.getByTestId('passkey-radio-platform').click();
    await page.getByRole('button', { name: /Continue/ }).click();

    await page.getByTestId('password.input').fill('iotae2etests');
    await page.getByTestId('password.confirmation').fill('iotae2etests');
    await page.getByText('I read and agree').click();
    await page.getByRole('button', { name: /Create Wallet/ }).click();

    await expect(page.getByText(username)).toBeVisible({ timeout: 10_000 });

    const { credentials: createdCredentials } = await client.send('WebAuthn.getCredentials', {
        authenticatorId,
    });

    expect(createdCredentials.length).toBeGreaterThan(0);
    expect(createdCredentials[0].rpId).toBe(EXPECTED_RP_ID);

    await page.getByTestId('wallet-settings-button').click();
    await page.getByText('Reset').click();
    await page.getByRole('button', { name: 'Reset' }).click();

    await page.getByRole('button', { name: /Get Started/ }).click({ timeout: SHORT_TIMEOUT });
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
