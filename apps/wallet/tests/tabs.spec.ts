// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { expect, test } from './fixtures';
import { createWallet } from './utils/auth';

test('Assets tab', async ({ page, extensionUrl }) => {
    await createWallet(page, extensionUrl);
    await page.getByTestId('nav-assets').click();

    await expect(page.getByRole('main')).toHaveText(/Assets/);
});

test('Apps tab', async ({ page, extensionUrl }) => {
    await createWallet(page, extensionUrl);
    await page.getByTestId('nav-apps').click();

    await expect(page.getByRole('main')).toHaveText(/Apps/i);
});

test('Activity tab', async ({ page, extensionUrl }) => {
    await createWallet(page, extensionUrl);
    await page.getByTestId('nav-activity').click();

    await expect(page.getByRole('main')).toHaveText(/Your Activity/);
});
