// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { expect, test } from './utils/fixtures';
import { BrowserContext, Page } from '@playwright/test';
import { connectWallet } from './utils/wallet';
import { SHORT_TIMEOUT } from './constants/timeout.constants';

interface TestContext {
    page: Page;
    pageWithFreshWalletPersistent: Page;
    context: BrowserContext;
}

test.describe.serial('Protected Routes', () => {
    test.setTimeout(20_000);
    const shared: TestContext = {} as TestContext;

    test.beforeAll(async ({ pageWithFreshWalletPersistent, persistentContext, sharedState }) => {
        shared.pageWithFreshWalletPersistent = pageWithFreshWalletPersistent;
        shared.context = persistentContext;
        const dashboardPage = await persistentContext.newPage();
        await dashboardPage.goto('/');

        await connectWallet(dashboardPage, persistentContext, sharedState.extension.name);

        shared.page = dashboardPage;
    });

    test('Assets route', async () => {
        const { page } = shared;
        await page.getByTestId('sidebar-assets').click();
        await expect(page.getByRole('heading', { name: 'Assets' })).toBeVisible({
            timeout: SHORT_TIMEOUT,
        });
    });

    test('Staking route', async () => {
        const { page } = shared;
        await page.getByTestId('sidebar-staking').click();
        await expect(page.getByText('Start Staking')).toBeVisible({
            timeout: SHORT_TIMEOUT,
        });
    });

    test('Activity route', async () => {
        const { page } = shared;
        await page.getByTestId('sidebar-activity').click();
        await expect(page.getByRole('heading', { name: 'Activity' })).toBeVisible({
            timeout: SHORT_TIMEOUT,
        });
    });

    test('Migration route', async () => {
        const { page } = shared;
        await page.getByTestId('sidebar-migration').click();
        await expect(page.getByRole('heading', { name: 'Migration' })).toBeVisible({
            timeout: SHORT_TIMEOUT,
        });
    });

    test('Vesting route', async () => {
        const { page } = shared;
        await page.getByTestId('sidebar-vesting').click();
        await expect(page.getByRole('heading', { name: 'Vesting' })).toBeVisible({
            timeout: SHORT_TIMEOUT,
        });
    });
    test.afterAll(async () => {
        if (shared.context && shared.context.browser()?.isConnected()) {
            await shared.context
                .close()
                .catch((e) => console.error('Error closing persistent context:', e));
        }
    });
});
