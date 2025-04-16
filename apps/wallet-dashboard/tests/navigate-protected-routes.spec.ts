// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Feature } from '@iota/core';
import { expect, test } from './fixtures';
import { connectWallet, createWallet } from './utils';
import { growthbook } from '@/lib/utils';
import { BrowserContext, Page } from '@playwright/test';

// Todo fix
// Configure the feature flags for the tests
const setupFeatureFlags = async (context: BrowserContext, page: Page) => {
    await context.addInitScript(() => {
        growthbook.getFeatureValue(Feature.StardustMigration, true);
        growthbook.getFeatureValue(Feature.SupplyIncreaseVesting, true);
    });

    await page.reload();
};

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
        await setupFeatureFlags(context, dashboardPage);

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

    // Todo fix

    // test('Migration route', async () => {
    //     await page.getByTestId('sidebar-migration').click();
    //     await expect(page.getByRole('heading', { name: 'Migration' })).toBeVisible({
    //         timeout: 30_000,
    //     });
    // });
    //
    // test('Vesting route', async () => {
    //     await page.getByTestId('sidebar-vesting').click();
    //     await expect(page.getByRole('heading', { name: 'Vesting' })).toBeVisible({
    //         timeout: 30_000,
    //     });
    // });
});
