// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Feature } from '@iota/core';
import { expect, test } from './fixtures';
import { connectWallet } from './utils';
import { growthbook } from '@/lib/utils';

// Configure the feature flags for the tests
const setupFeatureFlags = async (context: any, page: any) => {
    await context.addInitScript(() => {
        growthbook.getFeatureValue(Feature.StardustMigration, true);
        growthbook.getFeatureValue(Feature.SupplyIncreaseVesting, true);
    });
    await page.reload();
};

test.describe.serial('Protected Routes', () => {
    test.beforeEach(async ({ context, page, extensionName }) => {
        await setupFeatureFlags(context, page);
        await connectWallet(page, context, extensionName);
        await page.bringToFront();
    });
    test('Assets route', async ({ page }) => {
        await page.getByTestId('sidebar-assets').click();
        await expect(page.getByText('Assets')).toBeVisible({
            timeout: 30_000,
        });
    });

    test('Staking route', async ({ page }) => {
        await page.getByTestId('sidebar-staking').click();
        await expect(page.getByRole('button', { name: 'Stake' })).toBeVisible({
            timeout: 30_000,
        });
    });

    test('Activity route', async ({ page }) => {
        await page.getByTestId('sidebar-activity').click();
        await expect(page.getByTestId('activity-page')).toBeVisible({
            timeout: 30_000,
        });
    });

    test('Migration route', async ({ page }) => {
        await page.getByTestId('sidebar-migration').click();
        await expect(page.getByText('Migration')).toBeVisible({
            timeout: 30_000,
        });
    });

    test('Vesting route', async ({ page }) => {
        await page.getByTestId('sidebar-vesting').click();
        await expect(page.getByText('Vesting')).toBeVisible({
            timeout: 30_000,
        });
    });
});
