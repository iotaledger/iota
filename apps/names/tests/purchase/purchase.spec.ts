// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { normalizeIotaName } from '@iota/iota-names-sdk';
import { formatAddress } from '@iota/iota-sdk/utils';
import { expect } from '@playwright/test';

import { denormalizeName } from '@/lib/utils';

import { test } from '../helpers/fixtures';
import { addToCouponList } from '../setup/toggleSmartContract';
import {
    connectWallet,
    createWallet,
    generateRandomCoupon,
    generateRandomName,
    requestFaucetTokens,
} from '../utils';

test.describe.serial('Purchase Name Tests', () => {
    test.beforeAll(async ({ appPage, context, extensionPage, extensionName, sharedState }) => {
        const { address, mnemonic } = await createWallet(extensionPage);

        await appPage.bringToFront();

        await connectWallet(appPage, context, extensionName);

        await expect(appPage.getByRole('button', { name: formatAddress(address) })).toBeVisible({
            timeout: 10_000,
        });

        await requestFaucetTokens(address);

        sharedState.wallet.address = address;
        sharedState.wallet.mnemonic = mnemonic;
    });

    test('Can purchase a name making it public', async ({ appPage: page, context }) => {
        const nameToPurchase = `e2epublic`;

        await page.getByPlaceholder('Search for your IOTA name').filter({ visible: true }).click();

        await page.getByPlaceholder('Check name availability').fill(nameToPurchase);
        const purchaseCardLocator = page.getByTestId('purchase-name-card');

        await purchaseCardLocator.waitFor({ state: 'visible', timeout: 10_000 });
        await purchaseCardLocator.hover();
        await purchaseCardLocator.getByRole('button', { name: 'Buy' }).click({ timeout: 10_000 });
        await expect(page.getByTestId('name-purchase-title')).toContainText('@' + nameToPurchase);

        await page.getByText('Set name as Public Name').click();
        await page.getByRole('button', { name: 'Buy' }).click({ timeout: 10_000 });
        (await context.waitForEvent('page')).getByRole('button', { name: 'Approve' }).click();

        const normalizedName = normalizeIotaName(nameToPurchase + '.iota', 'at', {
            truncateLongParts: true,
        });
        await expect(page.getByText(`Successfully registered name ${normalizedName}`)).toBeVisible({
            timeout: 30_000,
        });

        await page.bringToFront();
    });

    test('Availability Dialog opens from all UI elements', async ({ appPage: page }) => {
        await page.goto('/', { waitUntil: 'domcontentloaded' });

        await page.getByPlaceholder('Search for your IOTA name').filter({ visible: true }).click();

        expect(page.getByRole('dialog').getByPlaceholder('Check name availability')).toBeVisible({
            timeout: 5_000,
        });

        await page.goto('/my-names', { waitUntil: 'domcontentloaded' });

        await page.getByRole('button', { name: 'Name', exact: true }).click();

        expect(page.getByRole('dialog').getByPlaceholder('Check name availability')).toBeVisible({
            timeout: 5_000,
        });
    });

    test('Unavailable shows up for already purchased name', async ({ appPage: page }) => {
        // Name registered when initializing localnet with scripts/tests/register-name.ts
        const targetName = 'test.iota';
        const denormalizedName = denormalizeName(targetName);

        await page.getByPlaceholder('Search for your IOTA name').click();
        await page.getByPlaceholder('Check name availability').fill(denormalizedName);

        const namePurchaseCard = page.getByTestId('unavailable-purchase-card');
        const nameFromCard = namePurchaseCard.getByTestId('name-purchase-card-name');
        await expect(nameFromCard).toContainText('@' + denormalizedName);
        const nameStatus = namePurchaseCard.getByTestId('name-purchase-card-status');
        await expect(nameStatus).toContainText(/Unavailable/i);
        const nameMessage = namePurchaseCard.getByTestId('name-purchase-card-status-message');
        await expect(nameMessage).toContainText('Name is already taken.');
    });

    test('Test with a coupon', async ({ appPage: page, context }) => {
        const couponCode = generateRandomCoupon('E2E100OFF');
        const name = generateRandomName('coupon');

        await addToCouponList(couponCode);
        await page.getByPlaceholder('Search for your IOTA name').click();
        await page.getByPlaceholder('Check name availability').fill(name);

        const purchaseCardLocator = page.getByTestId('purchase-name-card');

        await purchaseCardLocator.waitFor({ state: 'visible', timeout: 10_000 });
        await purchaseCardLocator.hover();
        await purchaseCardLocator.getByRole('button', { name: 'Buy' }).click({ timeout: 10_000 });
        await expect(page.getByTestId('name-purchase-title')).toContainText(
            '@' + denormalizeName(name),
        );

        await page.getByPlaceholder('Have a discount code?').fill(couponCode);
        await page.getByRole('button', { name: '+ Apply Coupon' }).click();

        await expect(page.getByRole('button', { name: couponCode })).toBeVisible({
            timeout: 5_000,
        });

        await page.getByRole('button', { name: 'Buy' }).click({ timeout: 10_000 });
        (await context.waitForEvent('page')).getByRole('button', { name: 'Approve' }).click();

        await expect(page.getByText('Successfully registered name')).toBeVisible({
            timeout: 30_000,
        });
    });

    test('Test with an invalid coupon', async ({ appPage: page, context }) => {
        const couponCode = generateRandomCoupon('E2E100OFF');
        const name = generateRandomName('invalid');

        await page.getByPlaceholder('Search for your IOTA name').click();
        await page.getByPlaceholder('Check name availability').fill(name);

        const purchaseCardLocator = page.getByTestId('purchase-name-card');

        await purchaseCardLocator.waitFor({ state: 'visible', timeout: 10_000 });
        await purchaseCardLocator.hover();
        await purchaseCardLocator.getByRole('button', { name: 'Buy' }).click({ timeout: 10_000 });
        await expect(page.getByTestId('name-purchase-title')).toContainText(
            '@' + denormalizeName(name),
        );

        await page.getByPlaceholder('Have a discount code?').fill(couponCode);
        await page.getByRole('button', { name: '+ Apply Coupon' }).click();

        await expect(page.getByText('Invalid coupon')).toBeVisible({
            timeout: 5_000,
        });
    });
    test('Buy with a different expiration time', async ({ appPage: page, context }) => {
        const YEARS = 5;
        // Name bought when initializing localnet with scripts/tests/register-name.ts
        const name = generateRandomName('expi');
        const denormalizedName = denormalizeName(name);

        await page.getByPlaceholder('Search for your IOTA name').click();
        await page.getByPlaceholder('Check name availability').fill(denormalizedName);

        const namePurchaseCard = page.getByTestId('purchase-name-card');
        await expect(namePurchaseCard).toContainText('@' + denormalizedName);

        await namePurchaseCard.hover();
        await namePurchaseCard.getByRole('button', { name: 'Buy' }).click();

        const initialExpirationTime = (
            await page.getByTestId('registration-expiration').innerText()
        ).split('\n')[1];

        const select = page.getByTestId('purchase-years-select');
        await expect(select).toContainText('1 Year');
        await select.click({ timeout: 5_000 });
        await page.getByText(`${YEARS} Years`).click({ timeout: 5_000 });

        const expirationTime = (
            await page.getByTestId('registration-expiration').innerText()
        ).split('\n')[1];

        expect(expirationTime).not.toBe(initialExpirationTime);
        const expirationYears = new Date(expirationTime).getFullYear() - new Date().getFullYear();
        expect(expirationYears).toBe(YEARS);

        await page.getByRole('button', { name: 'Buy' }).click();

        const walletConfirmationPage = context.waitForEvent('page');
        const walletPopup = await walletConfirmationPage;

        await walletPopup.waitForLoadState('domcontentloaded');
        const approveBtn = walletPopup.getByRole('button', { name: /^Approve$/i });
        await approveBtn.click();

        await walletPopup.waitForEvent('close', { timeout: 10_000 });
        await expect(page.getByText(/Successfully registered name/i)).toBeVisible({
            timeout: 5_000,
        });
    });
});
