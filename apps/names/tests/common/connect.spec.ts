// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { formatAddress } from '@iota/iota-sdk/utils';

import { expect, test } from '../helpers/fixtures';
import { connectWallet, createWallet } from '../utils';

test.describe('Connection tests', () => {
    test('should connect wallet to dapp', async ({
        context,
        extensionName,
        appPage,
        extensionPage,
        sharedState,
    }) => {
        const { address, mnemonic } = await createWallet(extensionPage);

        await appPage.bringToFront();

        await connectWallet(appPage, context, extensionName);

        await expect(appPage.getByRole('button', { name: formatAddress(address) })).toBeVisible({
            timeout: 10_000,
        });

        sharedState.wallet.address = address;
        sharedState.wallet.mnemonic = mnemonic;
    });
});
