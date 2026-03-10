// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { normalizeIotaName } from '@iota/iota-names-sdk';
import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';
import { formatAddress } from '@iota/iota-sdk/utils';

import { denormalizeName } from '@/lib/utils';

import { expect, test } from '../helpers/fixtures';
import {
    bidOnExistingAuction,
    connectWallet,
    createAndSendAuctionTransaction,
    createWallet,
    generateRandomName,
    requestFaucetTokens,
} from '../utils';
import { checkAuctionPills } from './auction.utils';

test.describe.serial('Auction Flow', () => {
    test.beforeAll(async ({ appPage, context, extensionPage, extensionName, sharedState }) => {
        const { address, mnemonic } = await createWallet(extensionPage);

        await appPage.bringToFront();

        await connectWallet(appPage, context, extensionName);
        await requestFaucetTokens(address);

        await expect(appPage.getByRole('button', { name: formatAddress(address) })).toBeVisible({
            timeout: 10_000,
        });

        sharedState.wallet.address = address;
        sharedState.wallet.mnemonic = mnemonic;
    });

    test('Start an auction', async ({ appPage: page }) => {
        const auctionName = generateRandomName('auction');
        const displayName = denormalizeName(auctionName);
        const initialSearch = page.getByPlaceholder('Search for your IOTA name');
        await initialSearch.click();

        const searchInput = page.getByPlaceholder('Check name availability');
        await expect(searchInput).toBeVisible({ timeout: 15_000 });

        await searchInput.fill(displayName);
        await page.keyboard.press('Enter');

        const auctionCard = page.getByTestId('auction-card').filter({
            has: page.getByRole('heading', { name: new RegExp(`^${displayName}$`, 'i') }),
        });
        await expect(auctionCard).toBeVisible();

        await auctionCard.hover();
        await auctionCard.getByRole('button', { name: 'Bid' }).click({ timeout: 10_000 });

        const dialog = page.getByRole('dialog').last();

        await expect(dialog.getByText('Auction', { exact: true })).toBeVisible({ timeout: 10_000 });
        const bidBtn = page.getByRole('button', { name: /^Start auction$/i });
        await expect(bidBtn).toBeVisible();
        const waitForWalletPopup = page.context().waitForEvent('page');
        await bidBtn.click();
        const walletPopup = await waitForWalletPopup;

        await walletPopup.waitForLoadState('domcontentloaded');

        const approveBtn = walletPopup.getByRole('button', { name: /^Approve$/i });
        await expect(approveBtn).toBeVisible({ timeout: 10_000 });
        await approveBtn.click();

        await walletPopup.waitForEvent('close', { timeout: 10_000 });
        await expect(page.getByText(/Successfully placed bid of/i)).toBeVisible({
            timeout: 15_000,
        });
        await page.goto(`/auctions?page=1&search=${displayName}`);
        const nameCard = page.getByTestId('name-card-body').filter({ hasText: displayName });
        await expect(nameCard).toBeVisible({ timeout: 10_000 });

        const bidAgainButton = nameCard.getByRole('button', { name: /Bid Again/i });
        await expect(bidAgainButton).toBeVisible();
    });

    test('Create bid on existing auction', async ({ appPage: page, context }) => {
        const auctionName = generateRandomName('existing');

        const keypair = new Ed25519Keypair();
        await requestFaucetTokens(keypair.toIotaAddress());

        const response = await createAndSendAuctionTransaction({
            signer: keypair,
            name: auctionName,
        });

        expect(response.effects?.status.status).toBe('success');

        await page.goto(`/auctions?page=1&search=${auctionName}`);
        await page.getByTestId('refresh-button').click({ timeout: 10_000 });

        await expect(page.getByText(/Refreshed successfully/i)).toBeVisible({
            timeout: 10_000,
        });

        const displayName = normalizeIotaName(auctionName, 'at');

        const nameCard = page.getByTestId('name-card-body').filter({ hasText: displayName });
        await expect(nameCard).toBeVisible({ timeout: 10_000 });

        await nameCard.getByRole('button', { name: /Bid/i }).click();
        const dialog = page.getByRole('dialog');
        await expect(dialog.getByText('Auction', { exact: true })).toBeVisible({ timeout: 15_000 });
        await page.getByRole('button', { name: /^Bid$/i }).click();
        const walletConfirmationPage = context.waitForEvent('page');
        const walletPopup = await walletConfirmationPage;

        await walletPopup.waitForLoadState('domcontentloaded');
        const approveBtn = walletPopup.getByRole('button', { name: /^Approve$/i });
        await approveBtn.click();

        await walletPopup.waitForEvent('close', { timeout: 10_000 });
        await expect(page.getByText(/Successfully placed bid of/i)).toBeVisible({
            timeout: 5_000,
        });
    });

    test('Claim an auction', async ({ sharedState, appPage: page, context }) => {
        test.setTimeout(60_000);
        const name = generateRandomName('claim');
        await page.bringToFront();
        await page.goto(`/auctions?page=1&search=${name}`);

        const keypair = Ed25519Keypair.deriveKeypair(sharedState.wallet.mnemonic!);
        await requestFaucetTokens(keypair.toIotaAddress());

        const response = await createAndSendAuctionTransaction({
            signer: keypair,
            name,
        });
        expect(response.effects?.status.status).toBe('success');

        await new Promise((resolve) => setTimeout(resolve, 1000));
        await page
            .locator("h2:has-text('Auctions')")
            .waitFor({ state: 'visible', timeout: 10_000 });

        await page.getByTestId('refresh-button').click();
        await page
            .getByText('Refreshed successfully')
            .waitFor({ state: 'visible', timeout: 10_000 });

        let auctionNameCard = page
            .getByTestId('name-card')
            .filter({ hasText: normalizeIotaName(name, 'at') });

        const timeRemainingLocator = auctionNameCard.getByTestId('auction-time-remaining');
        const textContent = await timeRemainingLocator.textContent();
        const secondsRemaining = Number(textContent?.replace('s', '').trim()) || 0;
        const offsetSeconds = 2;

        await new Promise((resolve) =>
            setTimeout(resolve, (offsetSeconds + secondsRemaining) * 1000),
        );

        await expect(timeRemainingLocator).toHaveText('Finished', { timeout: 10_000 });

        await page.getByTestId('refresh-button').click();
        await page
            .getByText('Refreshed successfully')
            .waitFor({ state: 'visible', timeout: 10_000 });

        auctionNameCard = page
            .getByTestId('name-card')
            .filter({ hasText: normalizeIotaName(name, 'at') });

        const claimButton = auctionNameCard.getByRole('button', { name: 'Claim' });
        await expect(claimButton).toBeEnabled({
            timeout: 10_000,
        });

        await claimButton.click();

        const approvePage = await context.waitForEvent('page', { timeout: 10_000 });

        await approvePage.waitForLoadState();
        await approvePage.bringToFront();
        await approvePage
            .getByText('Do you approve these actions?')
            .waitFor({ state: 'visible', timeout: 10_000 });

        await approvePage.getByRole('button', { name: 'Approve' }).click();
        await approvePage.waitForEvent('close', { timeout: 10_000 });
        await page.bringToFront();

        await expect(auctionNameCard.getByRole('button', { name: 'Claiming...' })).toBeVisible({
            timeout: 5_000,
        });

        await expect(auctionNameCard.getByText('Claimed')).toBeVisible({
            timeout: 20_000,
        });
    });

    test('Bid again on an auction', async ({ appPage: page, sharedState, context }) => {
        const auctionName = generateRandomName('again');
        const keypair = Ed25519Keypair.deriveKeypair(sharedState.wallet.mnemonic!);

        const response = await createAndSendAuctionTransaction({
            signer: keypair,
            name: auctionName,
        });

        expect(response.effects?.status.status).toBe('success');

        await page.goto(`/auctions?page=1&search=${auctionName}`);

        await page.getByTestId('refresh-button').click({ timeout: 10_000 });
        await expect(page.getByText(/Refreshed successfully/i)).toBeVisible();

        const displayName = normalizeIotaName(auctionName, 'at');

        const nameCard = page.getByTestId('name-card-body').filter({ hasText: displayName });
        await expect(nameCard).toBeVisible({ timeout: 10_000 });

        await nameCard.getByRole('button', { name: /Bid Again/i }).click();
        const dialog = page.getByRole('dialog');
        await expect(dialog.getByText('Auction', { exact: true })).toBeVisible({ timeout: 15_000 });
        await page.getByRole('button', { name: /^Bid$/i }).click();
        const walletConfirmationPage = context.waitForEvent('page');
        const walletPopup = await walletConfirmationPage;

        await walletPopup.waitForLoadState('domcontentloaded');
        const approveBtn = walletPopup.getByRole('button', { name: /^Approve$/i });
        await approveBtn.click();

        await walletPopup.waitForEvent('close', { timeout: 4000 });
        await expect(page.getByText(/Successfully placed bid of/i)).toBeVisible({
            timeout: 5_000,
        });
    });

    test('Check "Outbid" pills', async ({ sharedState, appPage: page }) => {
        const name = generateRandomName('outbid');

        const walletSigner = Ed25519Keypair.deriveKeypair(sharedState.wallet.mnemonic!);
        const newSigner = Ed25519Keypair.deriveKeypair(
            sharedState.wallet.mnemonic!,
            `m/44'/4218'/0'/0'/1'`,
        );

        await Promise.all([
            requestFaucetTokens(walletSigner.toIotaAddress()),
            requestFaucetTokens(newSigner.toIotaAddress()),
        ]);

        const startAuctionResult = await createAndSendAuctionTransaction({
            name,
            signer: walletSigner,
        });
        expect(startAuctionResult.effects?.status.status).toBe('success');

        const bidResult = await bidOnExistingAuction({
            name,
            signer: newSigner,
        });
        expect(bidResult.effects?.status.status).toBe('success');

        await checkAuctionPills(page, name, 'Outbid');
    });

    test('Check "Top Bidder" pills', async ({ sharedState, appPage: page }) => {
        const name = generateRandomName('topbid');

        const walletSigner = Ed25519Keypair.deriveKeypair(sharedState.wallet.mnemonic!);

        await requestFaucetTokens(walletSigner.toIotaAddress());

        const startAuctionResult = await createAndSendAuctionTransaction({
            name,
            signer: walletSigner,
        });
        expect(startAuctionResult.effects?.status.status).toBe('success');

        await checkAuctionPills(page, name, 'Top Bidder');
    });
});
