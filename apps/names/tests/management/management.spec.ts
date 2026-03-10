// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { resolve } from 'path';
import { normalizeIotaName } from '@iota/iota-names-sdk';
import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';
import { formatAddress } from '@iota/iota-sdk/utils';

import { formatDate } from '@/lib/utils/format/formatDate';

import { expect, test } from '../helpers/fixtures';
import { addToCouponList } from '../setup/toggleSmartContract';
import { iotaNamesClient } from '../setup/utils';
import {
    addSubnameName,
    connectAndSetPublicName,
    connectName,
    connectWallet,
    createWallet,
    editSetup,
    generateRandomCoupon,
    generateRandomName,
    generateRandomSubname,
    getAddressByIndexPath,
    mintNft,
    publishMovePackage,
    purchaseName,
    renewName,
    requestFaucetTokens,
    setAvatar,
} from '../utils';

test.describe.serial('Name Management Tests', () => {
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

    test('Add subname to a subname', async ({ appPage: page, context, sharedState }) => {
        const keypair = Ed25519Keypair.deriveKeypair(sharedState.wallet.mnemonic ?? '');
        const name = generateRandomName('addsubname');
        const subname = generateRandomSubname('subname', name);
        const responsePurchase = await purchaseName(name, keypair);
        expect(responsePurchase.effects?.status.status).toBe('success');

        const record = await iotaNamesClient.getNameRecord(name);
        if (!record) throw new Error('Name record not found');

        const responsePurchaseSubname = await addSubnameName(
            subname,
            record.nftId,
            record.expirationDate,
            keypair,
        );
        expect(responsePurchaseSubname.effects?.status.status).toBe('success');

        await page.goto('/my-names');
        await expect(
            page.getByTestId('name-card').filter({ hasText: normalizeIotaName(subname, 'at') }),
        ).toBeVisible({ timeout: 10_000 });

        const nameCard = page
            .getByTestId('name-card')
            .filter({ hasText: normalizeIotaName(subname, 'at') });

        await nameCard.getByTestId('name-card-avatar').hover();
        const menuButtonLocator = nameCard.getByTestId('menu-button');
        await expect(menuButtonLocator).toBeVisible();
        await menuButtonLocator.click();

        await page.getByText('Create Subname', { exact: true }).click();

        const dialog = page.getByRole('dialog');
        await expect(dialog.getByText('New Subname')).toBeVisible();

        await dialog.getByPlaceholder('Enter subname').fill('subname');

        await dialog.getByRole('button', { name: 'Create' }).click();
        (await context.waitForEvent('page')).getByRole('button', { name: 'Approve' }).click();
        await page.bringToFront();

        await expect(page.getByText('Successfully created subname', { exact: false })).toBeVisible({
            timeout: 30_000,
        });

        await page.close();
    });

    test('View name info', async ({ appPage: page, sharedState }) => {
        const keypair = Ed25519Keypair.deriveKeypair(sharedState.wallet.mnemonic ?? '');
        const name = generateRandomName('display');

        const response = await purchaseName(name, keypair);
        expect(response.effects?.status.status).toBe('success');

        await page.goto('/my-names');
        await expect(
            page.getByTestId('name-card').filter({ hasText: normalizeIotaName(name, 'at') }),
        ).toBeVisible({ timeout: 10_000 });

        const nameCard = page
            .getByTestId('name-card')
            .filter({ hasText: normalizeIotaName(name, 'at') });
        await nameCard.getByTestId('name-card-avatar').hover();
        const menuButtonLocator = nameCard.getByTestId('menu-button');
        await expect(menuButtonLocator).toBeVisible();
        await menuButtonLocator.click();

        await page.getByText('View All Info', { exact: true }).click();
        const dialog = page.getByRole('dialog');
        await expect(dialog.getByText('All Info')).toBeVisible();

        const record = await iotaNamesClient.getNameRecord(name);
        if (!record) throw new Error('Name record not found');
        const expectedOwner = formatAddress(sharedState.wallet.address ?? '');
        const expectedObjectId = formatAddress(record.nftId);
        const expectedExpirationText = formatDate(record.expirationDate);

        await expect(dialog.getByText('Owner', { exact: false })).toBeVisible();
        await expect(
            dialog.getByRole('link', {
                name: new RegExp(expectedOwner, 'i'),
            }),
        ).toBeVisible();
        await expect(dialog.getByText('Object ID', { exact: false })).toBeVisible();

        await expect(
            dialog.getByRole('link', {
                name: new RegExp(expectedObjectId, 'i'),
            }),
        ).toBeVisible();

        await expect(dialog.getByText('Expiration Time', { exact: false })).toBeVisible();
        await expect(dialog.getByText(expectedExpirationText)).toBeVisible();

        const closeIcon = page.getByTestId('close-icon');
        if (await closeIcon.isVisible().catch(() => false)) {
            await closeIcon.click();
        }

        await page.close();
    });

    test('Create subname', async ({ appPage: page, context, sharedState }) => {
        const keypair = Ed25519Keypair.deriveKeypair(sharedState.wallet.mnemonic ?? '');
        const name = generateRandomName('subname');
        const response = await purchaseName(name, keypair);
        expect(response.effects?.status.status).toBe('success');

        await page.goto('/my-names');
        await expect(
            page.getByTestId('name-card').filter({ hasText: normalizeIotaName(name, 'at') }),
        ).toBeVisible({ timeout: 10_000 });

        const nameCard = page
            .getByTestId('name-card')
            .filter({ hasText: normalizeIotaName(name, 'at') });
        await nameCard.getByTestId('name-card-avatar').hover();
        const menuButtonLocator = nameCard.getByTestId('menu-button');
        await expect(menuButtonLocator).toBeVisible();
        await menuButtonLocator.click();

        await page.getByText('Create Subname', { exact: true }).click();

        const dialog = page.getByRole('dialog');
        await expect(dialog.getByText('New Subname')).toBeVisible();

        await dialog.getByPlaceholder('Enter subname').fill('subname');

        await dialog.getByRole('button', { name: 'Create' }).click();
        (await context.waitForEvent('page')).getByRole('button', { name: 'Approve' }).click();
        await page.bringToFront();

        await expect(page.getByText('Successfully created subname', { exact: false })).toBeVisible({
            timeout: 30_000,
        });

        await page.close();
    });

    test('Connect address', async ({ appPage: page, context, sharedState }) => {
        const keypair = Ed25519Keypair.deriveKeypair(sharedState.wallet.mnemonic ?? '');
        const name = generateRandomName('connect');

        const responsePurchase = await purchaseName(name, keypair);
        expect(responsePurchase.effects?.status.status).toBe('success');

        await page.goto('/my-names');
        await expect(
            page.getByTestId('name-card').filter({ hasText: normalizeIotaName(name, 'at') }),
        ).toBeVisible({ timeout: 10_000 });

        await page.goto('/my-names');
        const nameCard = page
            .getByTestId('name-card')
            .filter({ hasText: normalizeIotaName(name, 'at') });
        await nameCard.getByTestId('name-card-avatar').hover();
        const menuButtonLocator = nameCard.getByTestId('menu-button');
        await expect(menuButtonLocator).toBeVisible();
        await menuButtonLocator.click();

        await page.getByText('Connect to Address', { exact: true }).click();
        const dialog = page.getByRole('dialog');
        await expect(dialog.getByText('Connect to Address')).toBeVisible();

        const mnemonic = sharedState.wallet.mnemonic as string;
        const externalAddress = getAddressByIndexPath(mnemonic, 1);

        await dialog.getByPlaceholder('Enter Address').fill(externalAddress);
        await dialog.getByRole('button', { name: 'Apply' }).click();
        (await context.waitForEvent('page')).getByRole('button', { name: 'Approve' }).click();
        await page.bringToFront();
        await expect(dialog.getByText('Address linked successfully')).toBeVisible();
        await dialog.getByRole('button', { name: 'Finish' }).click();

        await page.close();
    });

    test('Renew name', async ({ appPage: page, context, sharedState }) => {
        const keypair = Ed25519Keypair.deriveKeypair(sharedState.wallet.mnemonic ?? '');
        const name = generateRandomName('renew');

        const response = await purchaseName(name, keypair);
        expect(response.effects?.status.status).toBe('success');
        await page.goto('/my-names');
        await expect(
            page.getByTestId('name-card').filter({ hasText: normalizeIotaName(name, 'at') }),
        ).toBeVisible({ timeout: 10_000 });

        const nameCard = page
            .getByTestId('name-card')
            .filter({ hasText: normalizeIotaName(name, 'at') });
        await nameCard.getByTestId('name-card-avatar').hover();
        const menuButtonLocator = nameCard.getByTestId('menu-button');
        await expect(menuButtonLocator).toBeVisible();
        await menuButtonLocator.click();
        await page.getByText('Renew Name', { exact: true }).click();
        const dialog = page.getByRole('dialog');
        await expect(dialog.getByText('Renew Name', { exact: true })).toBeVisible();

        await dialog.getByRole('button', { name: 'Renew' }).click();
        (await context.waitForEvent('page')).getByRole('button', { name: 'Approve' }).click();
        await page.bringToFront();

        await expect(page.getByText('Name renewed successfully', { exact: false })).toBeVisible({
            timeout: 30_000,
        });

        await page.close();
    });

    test('Set permissions to a subname', async ({ appPage: page, context, sharedState }) => {
        const keypair = Ed25519Keypair.deriveKeypair(sharedState.wallet.mnemonic ?? '');
        const name = generateRandomName('perms');
        const subname = generateRandomSubname('subname', name);
        const responsePurchase = await purchaseName(name, keypair);
        expect(responsePurchase.effects?.status.status).toBe('success');

        const record = await iotaNamesClient.getNameRecord(name);
        if (!record) throw new Error('Name record not found');

        const responsePurchaseSubname = await addSubnameName(
            subname,
            record.nftId,
            record.expirationDate,
            keypair,
        );
        expect(responsePurchaseSubname.effects?.status.status).toBe('success');

        await page.goto('/my-names');
        await expect(
            page.getByTestId('name-card').filter({ hasText: normalizeIotaName(subname, 'at') }),
        ).toBeVisible({ timeout: 10_000 });

        const nameCard = page
            .getByTestId('name-card')
            .filter({ hasText: normalizeIotaName(subname, 'at') });

        await nameCard.getByTestId('name-card-avatar').hover();
        const menuButtonLocator = nameCard.getByTestId('menu-button');
        await expect(menuButtonLocator).toBeVisible();
        await menuButtonLocator.click();

        await page.getByText('Set Permissions', { exact: true }).click();
        const dialog = page.getByRole('dialog');
        await expect(dialog.getByText('Set permissions')).toBeVisible();

        const allowSubnamesLabel = dialog.getByText('Allow Subname to create additional Subnames');
        await expect(allowSubnamesLabel).toBeVisible();
        await allowSubnamesLabel.click();

        const allowRenewLabel = dialog.getByText('Allow Subname to renew expiration');
        await expect(allowRenewLabel).toBeVisible();
        await allowRenewLabel.click();

        const saveBtn = dialog.getByRole('button', { name: 'Save' });
        await expect(saveBtn).toBeEnabled();

        await dialog.getByRole('button', { name: 'Save' }).click();
        (await context.waitForEvent('page')).getByRole('button', { name: 'Approve' }).click();
        await page.bringToFront();

        await expect(
            page.getByText('Permissions updated successfully', { exact: false }),
        ).toBeVisible({ timeout: 30_000 });

        await page.close();
    });

    test('Can not add subname to a subname due permissions', async ({
        appPage: page,
        sharedState,
    }) => {
        const keypair = Ed25519Keypair.deriveKeypair(sharedState.wallet.mnemonic ?? '');
        const name = generateRandomName('nosubname');
        const subname = generateRandomSubname('subname', name);
        const responsePurchase = await purchaseName(name, keypair);
        expect(responsePurchase.effects?.status.status).toBe('success');

        const record = await iotaNamesClient.getNameRecord(name);
        if (!record) throw new Error('Name record not found');

        const responsePurchaseSubname = await addSubnameName(
            subname,
            record.nftId,
            record.expirationDate,
            keypair,
        );
        expect(responsePurchaseSubname.effects?.status.status).toBe('success');

        const responseEditSetup = await editSetup(subname, record.nftId, false, false, keypair);
        expect(responseEditSetup.effects?.status.status).toBe('success');

        await page.goto('/my-names');
        await expect(
            page.getByTestId('name-card').filter({ hasText: normalizeIotaName(subname, 'at') }),
        ).toBeVisible({ timeout: 10_000 });

        const nameCard = page
            .getByTestId('name-card')
            .filter({ hasText: normalizeIotaName(subname, 'at') });

        await nameCard.getByTestId('name-card-avatar').hover();
        const menuButtonLocator = nameCard.getByTestId('menu-button');
        await expect(menuButtonLocator).toBeVisible();
        await menuButtonLocator.click();

        await expect(page.getByText('Create Subname', { exact: true })).toHaveCount(0);

        // Method 2: Adding via subname counter
        await page.reload();

        const reloadedNameCard = page
            .getByTestId('name-card')
            .filter({ hasText: normalizeIotaName(subname, 'at') });
        await expect(reloadedNameCard).toBeVisible({ timeout: 10_000 });

        const subnamesCountLocator = reloadedNameCard.getByText('0 Subnames', { exact: true });
        await expect(subnamesCountLocator).toBeVisible({ timeout: 5_000 });
        await subnamesCountLocator.click();

        const newSubnameButton = page.getByRole('button', { name: 'New Subname' });
        await expect(newSubnameButton).toBeDisabled();

        // Method 3: Adding via parent subname counter
        await page.reload();

        const parentNameCard = page
            .getByTestId('name-card')
            .filter({ hasText: normalizeIotaName(name, 'at') })
            .filter({ has: page.getByText('1 Subname', { exact: true }) });

        await expect(parentNameCard).toBeVisible({ timeout: 5_000 });

        const parentCountLocator = parentNameCard.getByText('1 Subname', { exact: true });
        await expect(parentCountLocator).toBeVisible({ timeout: 5_000 });
        await parentCountLocator.click();

        const subnamesDialog = page.getByRole('dialog');
        await expect(subnamesDialog).toBeVisible();

        const subnameMenuButton = subnamesDialog.getByTestId('menu-button');
        await expect(subnameMenuButton).toBeVisible({ timeout: 5_000 });
        await subnameMenuButton.click();

        await expect(page.getByText('Create Subname', { exact: true })).toHaveCount(0);

        await page.close();
    });

    test('Disconnect address', async ({ appPage: page, context, sharedState }) => {
        const keypair = Ed25519Keypair.deriveKeypair(sharedState.wallet.mnemonic ?? '');
        const name = generateRandomName('disconnect');

        const responsePurchase = await purchaseName(name, keypair);
        expect(responsePurchase.effects?.status.status).toBe('success');

        const record = await iotaNamesClient.getNameRecord(name);
        if (!record) throw new Error('Name record not found');

        const response = await connectName(name, record.nftId, keypair);
        expect(response.effects?.status.status).toBe('success');

        await page.goto('/my-names');
        await expect(
            page.getByTestId('name-card').filter({ hasText: normalizeIotaName(name, 'at') }),
        ).toBeVisible({ timeout: 10_000 });

        const nameCard = page
            .getByTestId('name-card')
            .filter({ hasText: normalizeIotaName(name, 'at') });
        await nameCard.getByTestId('name-card-avatar').hover();
        const menuButtonLocator = nameCard.getByTestId('menu-button');
        await expect(menuButtonLocator).toBeVisible();
        await menuButtonLocator.click();

        await page.getByText('Connect to Address', { exact: true }).click();
        const dialog = page.getByRole('dialog');
        await expect(dialog.getByText('Connect to Address')).toBeVisible();

        await dialog.getByPlaceholder('Enter Address').fill('');
        await dialog.getByRole('button', { name: 'Apply' }).click();
        (await context.waitForEvent('page')).getByRole('button', { name: 'Approve' }).click();
        await page.bringToFront();

        await expect(page.getByText(`Successfully disconnected`, { exact: false })).toBeVisible({
            timeout: 30_000,
        });

        await page.close();
    });

    test('Renew subname', async ({ appPage: page, context, sharedState }) => {
        const keypair = Ed25519Keypair.deriveKeypair(sharedState.wallet.mnemonic ?? '');
        const name = generateRandomName('renewsub');
        const subname = generateRandomSubname('subname', name);
        const responsePurchase = await purchaseName(name, keypair);
        expect(responsePurchase.effects?.status.status).toBe('success');

        const record = await iotaNamesClient.getNameRecord(name);
        if (!record) throw new Error('Name record not found');

        const responsePurchaseSubname = await addSubnameName(
            subname,
            record.nftId,
            record.expirationDate,
            keypair,
        );
        expect(responsePurchaseSubname.effects?.status.status).toBe('success');

        const responseRenew = await renewName(name, record.nftId, keypair);
        expect(responseRenew.effects?.status.status).toBe('success');

        await page.goto('/my-names');
        await expect(
            page.getByTestId('name-card').filter({ hasText: normalizeIotaName(subname, 'at') }),
        ).toBeVisible({ timeout: 10_000 });

        const nameCard = page
            .getByTestId('name-card')
            .filter({ hasText: normalizeIotaName(subname, 'at') });

        await nameCard.getByTestId('name-card-avatar').hover();
        const menuButtonLocator = nameCard.getByTestId('menu-button');
        await expect(menuButtonLocator).toBeVisible();
        await menuButtonLocator.click();

        await page.getByText('Renew Subname', { exact: true }).click();
        const dialog = page.getByRole('dialog');
        await expect(dialog.getByText('Renew Subname', { exact: true })).toBeVisible();

        await dialog.getByRole('button', { name: 'Renew' }).click();
        (await context.waitForEvent('page')).getByRole('button', { name: 'Approve' }).click();
        await page.bringToFront();

        await expect(page.getByText('Subname renewed successfully', { exact: false })).toBeVisible({
            timeout: 30_000,
        });

        await page.close();
    });

    test('Can not renew a subname due permissions', async ({ appPage: page, sharedState }) => {
        const keypair = Ed25519Keypair.deriveKeypair(sharedState.wallet.mnemonic ?? '');
        const name = generateRandomName('norenew');
        const subname = generateRandomSubname('subname', name);
        const responsePurchase = await purchaseName(name, keypair);
        expect(responsePurchase.effects?.status.status).toBe('success');

        const record = await iotaNamesClient.getNameRecord(name);
        if (!record) throw new Error('Name record not found');

        const responsePurchaseSubname = await addSubnameName(
            subname,
            record.nftId,
            record.expirationDate,
            keypair,
        );
        expect(responsePurchaseSubname.effects?.status.status).toBe('success');

        const responseEditSetup = await editSetup(subname, record.nftId, false, false, keypair);
        expect(responseEditSetup.effects?.status.status).toBe('success');

        await page.goto('/my-names');
        await expect(
            page.getByTestId('name-card').filter({ hasText: normalizeIotaName(subname, 'at') }),
        ).toBeVisible({ timeout: 10_000 });

        const nameCard = page
            .getByTestId('name-card')
            .filter({ hasText: normalizeIotaName(subname, 'at') });

        await nameCard.getByTestId('name-card-avatar').hover();
        const menuButtonLocator = nameCard.getByTestId('menu-button');
        await expect(menuButtonLocator).toBeVisible();
        await menuButtonLocator.click();

        await expect(page.getByText('Renew Subname', { exact: true })).toHaveCount(0);

        // Method 2: Adding via parent subname counter
        await page.reload();

        const parentNameCard = page
            .getByTestId('name-card')
            .filter({ hasText: normalizeIotaName(name, 'at') })
            .filter({ has: page.getByText('1 Subname', { exact: true }) });

        await expect(parentNameCard).toBeVisible({ timeout: 5_000 });

        const parentCountLocator = parentNameCard.getByText('1 Subname', { exact: true });
        await expect(parentCountLocator).toBeVisible({ timeout: 5_000 });
        await parentCountLocator.click();

        const subnamesDialog = page.getByRole('dialog');
        await expect(subnamesDialog).toBeVisible();

        const subnameMenuButton = subnamesDialog.getByTestId('menu-button');
        await expect(subnameMenuButton).toBeVisible({ timeout: 5_000 });
        await subnameMenuButton.click();

        await expect(page.getByText('Renew Subname', { exact: true })).toHaveCount(0);
        await page.close();
    });

    test('Set name avatar', async ({ appPage: page, context, sharedState }) => {
        test.setTimeout(60_000);
        const keypair = Ed25519Keypair.deriveKeypair(sharedState.wallet.mnemonic ?? '');

        const name = generateRandomName('avatar');
        const response = await purchaseName(name, keypair);
        expect(response.effects?.status.status).toBe('success');

        const packagePath = resolve(__dirname, 'mint_nft');
        const { packageId } = await publishMovePackage(packagePath);
        console.log('[mint_nft publish] packageId (address):', packageId);
        expect(packageId.startsWith('0x')).toBeTruthy();

        const resultMint = await mintNft(packageId, keypair, {
            name: 'e2e test Avatar',
            description: 'E2E NFT',
            imageUrl: 'https://example.com/e2e.png',
        });
        expect(resultMint.effects?.status.status).toBe('success');

        await page.goto('/my-names');
        await expect(
            page.getByTestId('name-card').filter({ hasText: normalizeIotaName(name, 'at') }),
        ).toBeVisible({ timeout: 5_000 });
        const nameCard = page
            .getByTestId('name-card')
            .filter({ hasText: normalizeIotaName(name, 'at') });

        await nameCard.getByTestId('name-card-avatar').hover();
        const menuButtonLocator = nameCard.getByTestId('menu-button');
        await expect(menuButtonLocator).toBeVisible();
        await menuButtonLocator.click();

        // Wait slightly over the query staleTime (10s) to ensure a refetch happens
        await page.waitForTimeout(11_000);
        await page.getByText('Personalize Avatar', { exact: true }).click();
        const dialog = page.getByRole('dialog');
        await expect(dialog.getByText('Personalize Avatar', { exact: true })).toBeVisible();

        const mintedImg = dialog.getByRole('img', { name: 'e2e test Avatar' });
        await mintedImg.waitFor({ state: 'visible', timeout: 60_000 });
        const mintedCard = mintedImg.locator(
            'xpath=ancestor::*[@data-testid="avatar-nft-card"][1]',
        );
        await mintedCard.click();

        await dialog.getByRole('button', { name: 'Save' }).click();
        (await context.waitForEvent('page', { timeout: 60_000 }))
            .getByRole('button', { name: 'Approve' })
            .click();
        await page.bringToFront();

        await expect(
            page.getByText('Successfully updated avatar for ' + normalizeIotaName(name), {
                exact: false,
            }),
        ).toBeVisible({
            timeout: 60_000,
        });

        await page.close();
    });

    test('Unset name avatar', async ({ appPage: page, context, sharedState }) => {
        const keypair = Ed25519Keypair.deriveKeypair(sharedState.wallet.mnemonic ?? '');

        const name = generateRandomName('unset');
        const displayName = normalizeIotaName(name, 'at');

        const responsePurchase = await purchaseName(name, keypair);
        expect(responsePurchase.effects?.status.status).toBe('success');

        const record = await iotaNamesClient.getNameRecord(name);
        if (!record) throw new Error('Name record not found');

        const responseSetAvatar = await setAvatar(record, keypair);
        expect(responseSetAvatar.effects?.status.status).toBe('success');

        await page.goto('/my-names');
        await expect(page.getByTestId('name-card').filter({ hasText: displayName })).toBeVisible({
            timeout: 5_000,
        });
        const nameCard = page.getByTestId('name-card').filter({ hasText: displayName });

        await nameCard.getByTestId('name-card-avatar').hover();
        const menuButtonLocator = nameCard.getByTestId('menu-button');
        await expect(menuButtonLocator).toBeVisible();
        await menuButtonLocator.click();
        await page.getByText('Personalize Avatar', { exact: true }).click();
        const dialog = page.getByRole('dialog');
        await expect(dialog.getByText('Personalize Avatar', { exact: true })).toBeVisible();

        await dialog.getByRole('button', { name: 'Restore Default' }).click();
        await dialog.getByRole('button', { name: 'Save' }).click();
        (await context.waitForEvent('page')).getByRole('button', { name: 'Approve' }).click();
        await page.bringToFront();

        await expect(
            page.getByText('Successfully updated avatar for ' + displayName, {
                exact: false,
            }),
        ).toBeVisible({
            timeout: 10_000,
        });

        await page.close();
    });

    test('Unset Public Name', async ({ appPage: page, context, sharedState }) => {
        const keypair = Ed25519Keypair.deriveKeypair(sharedState.wallet.mnemonic ?? '');
        const name = generateRandomName('unset');
        const responsePurchase = await purchaseName(name, keypair);
        expect(responsePurchase.effects?.status.status).toBe('success');

        const record = await iotaNamesClient.getNameRecord(name);
        if (!record) throw new Error('Name record not found');

        const responseSetPublic = await connectAndSetPublicName(name, record.nftId, keypair);
        expect(responseSetPublic.effects?.status.status).toBe('success');

        await page.goto('/my-names');
        await expect(
            page.getByTestId('name-card').filter({ hasText: normalizeIotaName(name, 'at') }),
        ).toBeVisible({ timeout: 10_000 });

        const nameCard = page
            .getByTestId('name-card')
            .filter({ hasText: normalizeIotaName(name, 'at') });
        await nameCard.getByTestId('name-card-avatar').hover();
        const menuButtonLocator = nameCard.getByTestId('menu-button');
        await expect(menuButtonLocator).toBeVisible();
        await menuButtonLocator.click();

        await page.getByText('Connect to Address', { exact: true }).click();
        const dialog = page.getByRole('dialog');
        await expect(dialog.getByText('Connect to Address')).toBeVisible();
        await expect(dialog.getByText('Use as your public name')).toBeVisible();
        await expect(dialog.getByRole('checkbox')).toBeVisible();

        const enabledPublicNameCheckbox = dialog.getByRole('checkbox');
        await expect(enabledPublicNameCheckbox).toBeChecked({ checked: true, timeout: 10_000 });
        await dialog.getByText('Use as your public name').click();
        const disabledPublicNameCheckbox = dialog.getByRole('checkbox');
        await expect(disabledPublicNameCheckbox).toBeChecked({ checked: false, timeout: 10_000 });

        await dialog.getByRole('button', { name: 'Apply' }).click();

        (await context.waitForEvent('page')).getByRole('button', { name: 'Approve' }).click();
        await page.bringToFront();
        await expect(
            page.getByText(`${normalizeIotaName(name, 'at')} is no longer publicly visible.`),
        ).toBeVisible({
            timeout: 10_000,
        });
        await dialog.getByRole('button', { name: 'Finish' }).click();

        await page.close();
    });

    test('Renew name with coupon', async ({ appPage: page, context, sharedState }) => {
        const keypair = Ed25519Keypair.deriveKeypair(sharedState.wallet.mnemonic ?? '');
        const name = generateRandomName('renewcoupon');

        const responsePurchase = await purchaseName(name, keypair);
        expect(responsePurchase.effects?.status.status).toBe('success');

        const couponCode = generateRandomCoupon('E2E100OFF');
        await addToCouponList(couponCode);

        await page.goto('/my-names');
        await expect(
            page.getByTestId('name-card').filter({ hasText: normalizeIotaName(name, 'at') }),
        ).toBeVisible({ timeout: 10_000 });

        const nameCard = page
            .getByTestId('name-card')
            .filter({ hasText: normalizeIotaName(name, 'at') });
        await nameCard.getByTestId('name-card-avatar').hover();
        const menuButtonLocator = nameCard.getByTestId('menu-button');
        await expect(menuButtonLocator).toBeVisible();
        await menuButtonLocator.click();
        await page.getByText('Renew Name', { exact: true }).click();
        const dialog = page.getByRole('dialog');
        await expect(dialog.getByText('Renew Name', { exact: true })).toBeVisible();

        await dialog.getByPlaceholder('Have a discount code?').fill(couponCode);
        await dialog.getByText('+ Apply Coupon').click();
        await expect(dialog.getByText(couponCode, { exact: true })).toBeVisible();

        await dialog.getByRole('button', { name: 'Renew' }).click();
        (await context.waitForEvent('page')).getByRole('button', { name: 'Approve' }).click();
        await page.bringToFront();

        await expect(page.getByText('Name renewed successfully', { exact: false })).toBeVisible({
            timeout: 30_000,
        });

        await page.close();
    });

    test('Set Public Name', async ({ appPage: page, context, sharedState }) => {
        const keypair = Ed25519Keypair.deriveKeypair(sharedState.wallet.mnemonic ?? '');
        const name = generateRandomName('default');
        const responsePurchase = await purchaseName(name, keypair);
        expect(responsePurchase.effects?.status.status).toBe('success');

        const record = await iotaNamesClient.getNameRecord(name);
        if (!record) throw new Error('Name record not found');

        const responseConnect = await connectName(name, record.nftId, keypair);
        expect(responseConnect.effects?.status.status).toBe('success');

        await page.goto('/my-names');
        await expect(
            page.getByTestId('name-card').filter({ hasText: normalizeIotaName(name, 'at') }),
        ).toBeVisible({ timeout: 10_000 });

        const nameCard = page
            .getByTestId('name-card')
            .filter({ hasText: normalizeIotaName(name, 'at') });
        await nameCard.getByTestId('name-card-avatar').hover();
        const menuButtonLocator = nameCard.getByTestId('menu-button');
        await expect(menuButtonLocator).toBeVisible();
        await menuButtonLocator.click();

        await page.getByText('Connect to Address', { exact: true }).click();
        const dialog = page.getByRole('dialog');
        await expect(dialog.getByText('Connect to Address')).toBeVisible();

        await expect(dialog.getByText('Use as your public name')).toBeVisible();
        await dialog.getByText('Use as your public name').click();

        await dialog.getByRole('button', { name: 'Apply' }).click();
        (await context.waitForEvent('page')).getByRole('button', { name: 'Approve' }).click();
        await page.bringToFront();

        await expect(
            page.getByText(`${normalizeIotaName(name, 'at')} is now publicly visible.`, {
                exact: false,
            }),
        ).toBeVisible({
            timeout: 30_000,
        });
        await dialog.getByRole('button', { name: 'Finish' }).click();
        // Search 'Public Name' pill
        await page.getByTestId('refresh-button').click();
        const reloadedNameCard = page
            .getByTestId('name-card')
            .filter({ hasText: normalizeIotaName(name, 'at') });
        await expect(reloadedNameCard).toBeVisible({ timeout: 10_000 });

        const publicNamePill = reloadedNameCard.getByText('Public Name', { exact: true });
        await expect(publicNamePill).toBeVisible();

        await page.close();
    });
});
