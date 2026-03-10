// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { execFileSync } from 'child_process';
import { IotaNamesTransaction, isSubname, NameRecord } from '@iota/iota-names-sdk';
import { getNetwork } from '@iota/iota-sdk/client';
import type { Signer } from '@iota/iota-sdk/cryptography';
import { requestIotaFromFaucetV0 } from '@iota/iota-sdk/faucet';
import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';
import { Transaction } from '@iota/iota-sdk/transactions';
import { NANOS_PER_IOTA } from '@iota/iota-sdk/utils';
import type { BrowserContext, Page } from '@playwright/test';

import { buildCreateAuctionTransaction, buildPlaceBidTransaction } from '@/auctions';
import { CONFIG } from '@/config';

import { expect } from './helpers/fixtures';
import { iotaClientGraphQl, iotaNamesClient } from './setup/utils';

export async function connectWallet(page: Page, context: BrowserContext, extensionName: string) {
    await page.getByRole('button', { name: /Connect/i }).click();

    const pagePromise = context.waitForEvent('page', { timeout: 20_000 });
    await page.getByText(extensionName, { exact: true }).click();
    const walletApprovePage = await pagePromise;

    await walletApprovePage.waitForLoadState('load');
    await walletApprovePage.bringToFront();

    await walletApprovePage.getByRole('button', { name: 'Continue' }).click();

    await Promise.race([
        walletApprovePage
            .getByRole('button', { name: 'Connect' })
            .click()
            .catch(() => {}),
        walletApprovePage.waitForEvent('close'),
    ]);

    await page.bringToFront();

    await expect(page.getByRole('button', { name: 'Connect' })).not.toBeVisible({
        timeout: 5_000,
    });
}

export async function createWallet(page: Page) {
    await page.bringToFront();
    await page.getByRole('button', { name: /Get Started/ }).click({ timeout: 30_000 });
    await page.getByText('Create a new wallet').click();
    await page.getByText('Mnemonic', { exact: true }).click();
    await page.getByTestId('password.input').fill('iotae2etests');
    await page.getByTestId('password.confirmation').fill('iotae2etests');
    await page.getByText('I read and agree').click();

    await page.getByRole('button', { name: /Create Wallet/ }).click();
    await page.waitForURL(new RegExp(/accounts\/backup/));

    const BOX_TEST_ID = 'mnemonic-display-box';
    const mnemonicBox = page.getByTestId(BOX_TEST_ID);

    await expect(mnemonicBox).toBeVisible();

    await mnemonicBox.getByRole('button').first().click();
    const textarea = mnemonicBox.locator('textarea');
    const mnemonic = await textarea.inputValue();

    const address = deriveAddressFromMnemonic(mnemonic);

    await page.getByText('I saved my mnemonic').click();
    await page.getByRole('button', { name: 'Open Wallet' }).click();

    await page.getByLabel('Open settings menu').click();
    await page.getByText('Network').click();
    await page.getByText('Custom RPC').click();
    const networkId = CONFIG.network;
    const networkConfig = getNetwork(networkId);
    await page.getByPlaceholder('http://localhost:3000/').fill(networkConfig.url);
    await page.getByRole('button', { name: 'Save' }).click();

    await page.getByTestId('close-icon').click();

    return {
        mnemonic,
        address,
    };
}

export async function requestFaucetTokens(recipient: string, blockUntilBalance: boolean = true) {
    const currentNetwork = CONFIG.network;
    const networkConfig = getNetwork(currentNetwork);

    const faucetUrl = networkConfig.faucet;
    if (!faucetUrl) {
        throw new Error(`Faucet URL not defined for network: ${currentNetwork}`);
    }

    console.log(`Requesting faucet tokens from ${faucetUrl} to address: ${recipient}`);
    const res = await requestIotaFromFaucetV0({
        host: faucetUrl,
        recipient,
    });

    if (res.error) {
        throw new Error(`Faucet error: ${res.error}`);
    }

    if (blockUntilBalance) {
        await checkAddressBalanceWithRetries(recipient);
    }
}

export async function purchaseName(name: string, signer: Signer) {
    const address = signer.toIotaAddress();
    const tx = new Transaction();
    const iotaNamesTx = new IotaNamesTransaction(iotaNamesClient, tx);
    const [coin] = iotaNamesTx.transaction.splitCoins(tx.gas, [50_000_000_000]);
    const nft = await iotaNamesTx.register({
        name,
        coin,
        address,
    });
    iotaNamesTx.transaction.transferObjects([nft, coin], address);
    iotaNamesTx.transaction.setSender(address);
    const txBytes = await iotaNamesTx.transaction.build({
        client: iotaClientGraphQl,
    });
    const responsePurchase = await iotaClientGraphQl.signAndExecuteTransaction({
        transaction: txBytes,
        signer,
        options: {
            showEffects: true,
        },
    });

    await iotaClientGraphQl.waitForTransaction({ digest: responsePurchase.digest });

    console.log(`Purchased name: ${name} with address: ${address}`);
    return responsePurchase;
}
export async function addSubnameName(
    subname: string,
    parentNftId: string,
    expirationDate: Date,
    signer: Signer,
) {
    const address = signer.toIotaAddress();
    const tx = new Transaction();
    const iotaNamesTx = new IotaNamesTransaction(iotaNamesClient, tx);
    const subnameNft = await iotaNamesTx.createSubname({
        parentNft: tx.object(parentNftId),
        name: subname,
        expirationTimestampMs: expirationDate.getTime(),
        allowChildCreation: true,
        allowTimeExtension: true,
    });
    iotaNamesTx.transaction.transferObjects([subnameNft], address);
    iotaNamesTx.transaction.setSender(address);
    const txBytes = await iotaNamesTx.transaction.build({
        client: iotaClientGraphQl,
    });
    const responsePurchaseSubname = await iotaClientGraphQl.signAndExecuteTransaction({
        transaction: txBytes,
        signer,
        options: {
            showEffects: true,
        },
    });

    await iotaClientGraphQl.waitForTransaction({ digest: responsePurchaseSubname.digest });

    console.log(`Purchased subname: ${subname} with address: ${address}`);
    return responsePurchaseSubname;
}

export async function editSetup(
    subname: string,
    parentNftId: string,
    allowChildCreation: boolean,
    allowTimeExtension: boolean,
    signer: Signer,
) {
    const address = signer.toIotaAddress();
    const tx = new Transaction();
    const iotaNamesTx = new IotaNamesTransaction(iotaNamesClient, tx);
    iotaNamesTx.editSetup({
        parentNft: tx.object(parentNftId),
        name: subname,
        allowChildCreation,
        allowTimeExtension,
    });
    iotaNamesTx.transaction.setSender(address);
    const txBytes = await iotaNamesTx.transaction.build({
        client: iotaClientGraphQl,
    });
    const responseEditSetup = await iotaClientGraphQl.signAndExecuteTransaction({
        transaction: txBytes,
        signer,
        options: {
            showEffects: true,
        },
    });

    await iotaClientGraphQl.waitForTransaction({ digest: responseEditSetup.digest });

    console.log(
        `Edit permissions of subname: ${subname} with permissions: allowCreateChildren: ${allowChildCreation}, allowTimeExtension: ${allowTimeExtension}`,
    );
    return responseEditSetup;
}

export async function connectName(name: string, nft: string, signer: Signer) {
    const address = signer.toIotaAddress();
    const tx = new Transaction();
    const iotaNamesTx = new IotaNamesTransaction(iotaNamesClient, tx);
    iotaNamesTx.setTargetAddress({
        nft,
        address,
        isSubname: false,
    });
    iotaNamesTx.transaction.setSender(address);
    const txBytes = await iotaNamesTx.transaction.build({
        client: iotaClientGraphQl,
    });
    const responseConnect = await iotaClientGraphQl.signAndExecuteTransaction({
        transaction: txBytes,
        signer,
        options: {
            showEffects: true,
        },
    });

    await iotaClientGraphQl.waitForTransaction({ digest: responseConnect.digest });

    console.log(`Connected name: ${name} to address: ${address}`);
    return responseConnect;
}

export async function renewName(name: string, parentNftId: string, signer: Signer) {
    const address = signer.toIotaAddress();
    const tx = new Transaction();
    const iotaNamesTx = new IotaNamesTransaction(iotaNamesClient, tx);
    await iotaNamesTx.renew({
        nft: parentNftId,
        name: name,
        years: 1,
        coin: tx.gas,
        address: address,
    });
    iotaNamesTx.transaction.setSender(address);
    const txBytes = await iotaNamesTx.transaction.build({
        client: iotaClientGraphQl,
    });

    const responseRenew = await iotaClientGraphQl.signAndExecuteTransaction({
        transaction: txBytes,
        signer,
        options: {
            showEffects: true,
        },
    });

    await iotaClientGraphQl.waitForTransaction({ digest: responseRenew.digest });

    console.log(`Renewed name: ${name} with address: ${address}`);
    return responseRenew;
}

export async function publishMovePackage(packagePath: string) {
    const cliOutput = execFileSync('iota', ['client', 'publish', packagePath], {
        encoding: 'utf-8',
    });
    const pkgMatch = cliOutput.match(/PackageID:\s*(0x[0-9a-fA-F]+)/);
    const digestMatch = cliOutput.match(/Transaction Digest:\s*([A-Za-z0-9]+)/);
    if (!pkgMatch) throw new Error('Failed to parse packageId from CLI output');
    const packageId = pkgMatch[1];
    const digest = digestMatch ? digestMatch[1] : 'UNKNOWN';
    console.log('[publishMovePackage] CLI publish packageId:', packageId);

    if (/DisplayCreated<.*::mint_nft::Nft>/.test(cliOutput)) {
        console.log('[publishMovePackage] Display object detected (CLI)');
    } else {
        console.log('[publishMovePackage] No Display object detected in CLI output');
    }
    return { packageId, digest, result: { cliOutput } as unknown };
}

export async function mintNft(
    packageId: string,
    signer: Signer,
    {
        name = 'Test NFT',
        description = 'E2E Minted NFT',
        imageUrl = 'https://example.com/image.png',
    }: {
        name?: string;
        description?: string;
        imageUrl?: string;
    } = {},
) {
    const tx = new Transaction();
    const sender = signer.toIotaAddress();
    tx.setSender(sender);
    tx.moveCall({
        target: `${packageId}::mint_nft::mint`,
        arguments: [tx.pure.string(name), tx.pure.string(description), tx.pure.string(imageUrl)],
    });
    const built = await tx.build({ client: iotaClientGraphQl });
    const resultMint = await iotaClientGraphQl.signAndExecuteTransaction({
        transaction: built,
        signer,
        options: { showEffects: true, showObjectChanges: true },
    });
    if (resultMint.effects?.status.status !== 'success') {
        throw new Error(resultMint.effects?.status.error || 'Mint execution failed');
    }

    await iotaClientGraphQl.waitForTransaction({ digest: resultMint.digest });

    return resultMint;
}

export async function setAvatar(nameRecord: NameRecord, signer: Signer) {
    const address = signer.toIotaAddress();
    const isNameSubname = nameRecord.name ? isSubname(nameRecord.name) : false;
    const tx = new Transaction();
    const iotaNamesTx = new IotaNamesTransaction(iotaNamesClient, tx);
    iotaNamesTx.setUserData({
        nft: nameRecord.nftId,
        key: 'avatar',
        value: nameRecord.avatar ?? '0x0',
        isSubname: isNameSubname,
    });
    iotaNamesTx.transaction.setSender(address);
    const txBytes = await iotaNamesTx.transaction.build({
        client: iotaClientGraphQl,
    });

    const responseSetAvatar = await iotaClientGraphQl.signAndExecuteTransaction({
        transaction: txBytes,
        signer,
        options: {
            showEffects: true,
        },
    });

    await iotaClientGraphQl.waitForTransaction({ digest: responseSetAvatar.digest });

    console.log(`Avatar set to address: ${address}`);
    return responseSetAvatar;
}

export async function connectAndSetPublicName(name: string, nft: string, signer: Signer) {
    const address = signer.toIotaAddress();
    const tx = new Transaction();
    const iotaNamesTx = new IotaNamesTransaction(iotaNamesClient, tx);
    iotaNamesTx.setTargetAddress({
        nft,
        address,
        isSubname: false,
    });
    iotaNamesTx.setPublic(name);
    iotaNamesTx.transaction.setSender(address);
    const txBytes = await iotaNamesTx.transaction.build({
        client: iotaClientGraphQl,
    });

    const responseSetPublic = await iotaClientGraphQl.signAndExecuteTransaction({
        transaction: txBytes,
        signer,
        options: {
            showEffects: true,
        },
    });
    await iotaClientGraphQl.waitForTransaction({ digest: responseSetPublic.digest });
    console.log(`Set name: ${name} as public name for address: ${address}`);
    return responseSetPublic;
}

export function deriveAddressFromMnemonic(mnemonic: string, path?: string) {
    const keypair = Ed25519Keypair.deriveKeypair(mnemonic, path);
    const address = keypair.getPublicKey().toIotaAddress();
    return address;
}

export function getAddressByIndexPath(mnemonic: string, index: number) {
    return deriveAddressFromMnemonic(mnemonic, `m/44'/4218'/0'/0'/${index}'`);
}

interface CreateAndSendAuctionTransaction {
    name: string;
    signer: Signer;
    bidAmountIota?: bigint;
}
export async function createAndSendAuctionTransaction({
    name,
    signer,
    bidAmountIota = BigInt(50),
}: CreateAndSendAuctionTransaction) {
    try {
        const tx = buildCreateAuctionTransaction(
            iotaNamesClient.getPackage('auctionPackageId'),
            iotaNamesClient.getPackage('iotaNamesObjectId'),
            iotaNamesClient.getPackage('auctionHouseObjectId'),
            signer.toIotaAddress(),
            bidAmountIota * NANOS_PER_IOTA,
            name,
        );

        const txBytes = await tx.build({ client: iotaClientGraphQl });
        const response = await iotaClientGraphQl.signAndExecuteTransaction({
            transaction: txBytes,
            signer,
            options: {
                showEffects: true,
            },
        });

        await iotaClientGraphQl.waitForTransaction({ digest: response.digest });

        console.log('Transaction sent. Digest:', response.digest);
        console.log(`Successfully created auction for name: ${name}`);

        return response;
    } catch (error) {
        console.error('Error creating initial auction:', error);
        throw error;
    }
}

interface BidOnExistingAuction {
    name: string;
    signer: Signer;
    bidAmountIota?: bigint;
}
export async function bidOnExistingAuction({
    name,
    signer,
    bidAmountIota = BigInt(51),
}: BidOnExistingAuction) {
    try {
        const tx = buildPlaceBidTransaction(
            iotaNamesClient.getPackage('auctionPackageId'),
            iotaNamesClient.getPackage('auctionHouseObjectId'),
            signer.toIotaAddress(),
            bidAmountIota * NANOS_PER_IOTA,
            name,
        );

        const txBytes = await tx.build({ client: iotaClientGraphQl });
        const response = await iotaClientGraphQl.signAndExecuteTransaction({
            transaction: txBytes,
            signer,
            options: {
                showEffects: true,
            },
        });

        await iotaClientGraphQl.waitForTransaction({ digest: response.digest });

        console.log('Transaction sent. Digest:', response.digest);
        console.log(`Successfully bid on existing auction for name: ${name}`);

        return response;
    } catch (error) {
        console.error('Error bidding on auction:', error);
        throw error;
    }
}

export function generateRandomName(name: string) {
    const random = Math.floor(Math.random() * 10_000);
    return `${name}${random}.iota`;
}

export function generateRandomSubname(subname: string, parentName: string) {
    const random = Math.floor(Math.random() * 10_000);
    return `${subname}${random}.${parentName}`;
}

export function generateRandomCoupon(coupon: string) {
    const random = Math.floor(Math.random() * 10_000);
    return `${coupon}${random}`.toUpperCase();
}

export async function checkAddressBalanceWithRetries(address: string): Promise<void> {
    for (let attempt = 1; attempt <= 5; attempt++) {
        let totalBalance: string | null = null;

        try {
            const balanceResponse = await iotaClientGraphQl.getBalance({ owner: address });
            totalBalance = balanceResponse.totalBalance;
        } catch (error) {
            console.error(`Error checking balance for address ${address}:`, error);
        }

        if (totalBalance && !totalBalance.startsWith('0')) {
            console.log(`Address ${address} has balance: ${totalBalance} IOTA`);
            break;
        }

        console.log(
            `Attempt ${attempt}: Address ${address} balance is zero. Retrying in 2 seconds...`,
        );
        await new Promise((resolve) => setTimeout(resolve, 2000));
    }
}
