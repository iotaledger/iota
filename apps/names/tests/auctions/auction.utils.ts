// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { normalizeIotaName } from '@iota/iota-names-sdk';
import { Page } from '@playwright/test';

import { expect } from '../helpers/fixtures';

export async function checkAuctionPills(
    page: Page,
    nameToAuction: string,
    status: 'Top Bidder' | 'Outbid',
) {
    await page.bringToFront();

    await page.goto(`/my-names`);
    await page.locator("h2:has-text('My Names')").waitFor({ state: 'visible', timeout: 10_000 });

    await page.getByTestId('refresh-button').click();
    await page.getByText('Refreshed successfully').waitFor({ state: 'visible', timeout: 10_000 });

    const auctionNameCard = page
        .getByTestId('name-card')
        .filter({ hasText: normalizeIotaName(nameToAuction, 'at') });

    await expect(auctionNameCard.getByTestId('auction-status-badge')).toHaveText(status, {
        timeout: 10_000,
    });

    await page.getByRole('link', { name: 'Auctions' }).click();
    await page.getByPlaceholder('Search auction').fill(nameToAuction);
    const auctionCard = page.getByTestId('name-card').filter({
        hasText: normalizeIotaName(nameToAuction, 'at'),
    });

    await expect(auctionCard).toBeVisible({ timeout: 10_000 });
    await expect(auctionCard.getByTestId('auction-status-badge')).toHaveText(status, {
        timeout: 10_000,
    });

    await page.getByLabel('Go to homepage').filter({ visible: true }).first().click();
    await page.getByText('Live Auctions').scrollIntoViewIfNeeded();

    const carousel = page.getByTestId('auction-carousel');
    const auctionInCarousel = carousel
        .getByTestId('name-card')
        .filter({
            hasText: normalizeIotaName(nameToAuction, 'at'),
        })
        .first();
    await expect(auctionInCarousel.getByTestId('auction-status-badge')).toHaveText(status, {
        timeout: 10_000,
    });
}
