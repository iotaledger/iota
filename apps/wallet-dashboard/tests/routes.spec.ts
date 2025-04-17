// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { expect, test } from './fixtures';
import { connectWallet, createWallet } from './utils';
import { Page } from '@playwright/test';

test.describe.serial('Protected Routes', () => {
    test.setTimeout(20_000);
    let page: Page;

    test.beforeAll(async ({ context, extensionName, extensionUrl }) => {
        const extensionPage = await context.newPage();
        await extensionPage.goto(extensionUrl);

        await createWallet(extensionPage);

        const dashboardPage = await context.newPage();
        await dashboardPage.goto('/');

        await connectWallet(dashboardPage, context, extensionName);

        page = dashboardPage;
    });

    test('Assets route', async () => {
        await page.getByTestId('sidebar-assets').click();
        await expect(page.getByRole('heading', { name: 'Assets' })).toBeVisible({
            timeout: 30_000,
        });
    });

    test('Staking route', async () => {
        await page.getByTestId('sidebar-staking').click();
        await expect(page.getByText('Start Staking')).toBeVisible({
            timeout: 30_000,
        });
    });

    test('Activity route', async () => {
        await page.getByTestId('sidebar-activity').click();
        await expect(page.getByRole('heading', { name: 'Activity' })).toBeVisible({
            timeout: 30_000,
        });
    });

    test('Migration route', async () => {
        await page.getByTestId('sidebar-migration').click();
        await expect(page.getByRole('heading', { name: 'Migration' })).toBeVisible({
            timeout: 30_000,
        });
    });

    test('Vesting route', async () => {
        await page.getByTestId('sidebar-vesting').click();
        await expect(page.getByRole('heading', { name: 'Vesting' })).toBeVisible({
            timeout: 30_000,
        });
    });
});
