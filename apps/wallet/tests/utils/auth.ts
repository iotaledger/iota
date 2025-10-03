// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { CDPSession, Page } from '@playwright/test';
import { SHORT_TIMEOUT } from '../constants/timeout.constants';
import { expect } from '../fixtures';

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

    const mnemonicArray = typeof mnemonic === 'string' ? mnemonic.split(' ') : mnemonic;

    if (mnemonicArray.length === 12) {
        await page.locator('button:has(div:has-text("24 words"))').click();
        await page.getByText('12 words').click();
    }
    const wordInputs = await page.locator('input[placeholder="Word"]');
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

    if (await page.getByText('Balance Finder').isVisible()) {
        await page.getByRole('button', { name: /Skip/ }).click();
    }
}
interface VirtualAuthenticatorOptions {
    isCrossPlatform?: boolean;
    /**
     * Whether the authenticator should automatically respond to requests for user presence.
     * Defaults to true.
     */
    automaticPresenceSimulation?: boolean;
}
export async function addVirtualAuthenticator(
    client: CDPSession,
    options: VirtualAuthenticatorOptions = {},
) {
    return await client.send('WebAuthn.addVirtualAuthenticator', {
        options: {
            protocol: 'ctap2',
            transport: options.isCrossPlatform ? 'usb' : 'internal',
            hasResidentKey: true,
            hasUserVerification: true,
            isUserVerified: true,
            automaticPresenceSimulation: options.automaticPresenceSimulation ?? true,
        },
    });
}

interface PasskeyOptions extends VirtualAuthenticatorOptions {
    username: string;
    displayName: string;
}
export async function createPasskeyWallet(
    page: Page,
    extensionUrl: string,
    { username, displayName, automaticPresenceSimulation, isCrossPlatform }: PasskeyOptions,
) {
    const client = await page.context().newCDPSession(page);
    await client.send('WebAuthn.enable');
    const { authenticatorId } = await addVirtualAuthenticator(client, {
        automaticPresenceSimulation,
        isCrossPlatform,
    });

    await page.goto(extensionUrl, { waitUntil: 'commit' });
    await page.getByRole('button', { name: /Add Profile/ }).click({ timeout: SHORT_TIMEOUT });
    await page.getByText('Passkey', { exact: true }).click();

    await page.getByTestId('username-input').fill(username);
    await page.getByTestId('display-name-input').fill(displayName);

    if (isCrossPlatform) {
        await page.getByText('Platform').click();
        await expect(page.getByText('Cross-Platform')).toBeVisible();
    }

    await page.getByRole('button', { name: /Create/ }).click();

    await page.getByTestId('password.input').fill('iotae2etests');
    await page.getByTestId('password.confirmation').fill('iotae2etests');

    await page.getByText('I read and agree').click();

    await page.getByRole('button', { name: /Create Wallet/ }).click();

    return {
        client,
        authenticatorId,
    };
}
