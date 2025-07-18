import { IotaClient, CoinStruct } from '@iota/iota-sdk/client';
import { requestIotaFromFaucetV0 } from '@iota/iota-sdk/faucet';
import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';
import { IOTA_TYPE_ARG, IOTA_DECIMALS } from '@iota/iota-sdk/utils';
import { parseAmount } from '../../src/lib/utils/parseAmount';
import { createDepositTransactionL1 } from '../../src/lib/utils/transaction/createDepositTransactionL1';
import { CONFIG } from '../config/config';
import { MNEMONIC, THREE_MINUTES, TOOL_COIN_OBJECT_ID, TOOL_COIN_TYPE } from '../utils/constants';
import { Transaction } from '@iota/iota-sdk/transactions';
import { bcs } from '@iota/iota-sdk/bcs';
import { BrowserContext, expect, Page } from '@playwright/test';
import { getExtensionUrl } from './browser';

export async function fundL1AddressWithNativeTokens(addressL1: string, amount: number) {
    const { L1 } = CONFIG;

    const client = new IotaClient({
        url: L1.rpcUrl,
    });

    const keypair = Ed25519Keypair.deriveKeypair(MNEMONIC);
    const address = keypair.toIotaAddress();

    const tx = new Transaction();

    const tokenCoin = tx.splitCoins(tx.object(TOOL_COIN_OBJECT_ID), [
        tx.pure(bcs.U64.serialize(amount)),
    ]);
    tx.transferObjects([tokenCoin], addressL1);
    tx.setSender(address);

    await client.signAndExecuteTransaction({
        signer: keypair,
        transaction: tx,
    });
}

export async function fundL2AddressWithIscClient(
    addressL2: string,
    amount: number,
    coinType = IOTA_TYPE_ARG,
) {
    const { L1 } = CONFIG;
    const chain = {
        chainId: L1.chainId,
        packageId: L1.packageId,
    };

    const client = new IotaClient({
        url: L1.rpcUrl,
    });
    const coinData = await client.getCoinMetadata({ coinType });
    const amountToSend = parseAmount(
        amount.toString(),
        coinData?.decimals ?? IOTA_DECIMALS,
    ) as bigint;

    const keypair =
        coinType === IOTA_TYPE_ARG ? new Ed25519Keypair() : Ed25519Keypair.deriveKeypair(MNEMONIC);
    const address = keypair.toIotaAddress();
    let coins: CoinStruct[] = [];

    if (coinType === IOTA_TYPE_ARG) {
        await requestIotaFromFaucetV0({
            host: L1.faucetUrl!,
            recipient: address,
        });
    } else {
        const { data: toolCoins } = await client.getCoins({
            coinType: TOOL_COIN_TYPE,
            owner: address,
        });
        coins = toolCoins;
    }

    const transaction = createDepositTransactionL1({
        amount: amountToSend,
        receivingAddress: addressL2,
        coins,
        coinType,
        chain,
    });
    transaction.setSender(address);
    await transaction.build({ client });

    await client.signAndExecuteTransaction({
        signer: keypair,
        transaction,
    });
}

export async function addL1FundsThroughBridgeUI(page: Page, browser: BrowserContext) {
    const maxRetries = 3; // Maximum number of retry attempts
    let attempt = 1;
    let success = false;

    while (attempt <= maxRetries && !success) {
        try {
            console.log(`Attempt ${attempt}/${maxRetries} to add funds through bridge UI`);

            // Add funds to L1
            await page.getByTestId('request-l1-funds-button').click();

            // Wait for transaction completion - look for either success or error message
            const successPromise = page
                .getByText('Funds successfully sent.')
                .waitFor({ timeout: 30000 })
                .then(() => 'success')
                .catch(() => 'timeout');

            const errorPromise = page
                .getByText('Something went wrong while requesting funds.')
                .waitFor({ timeout: 30000 })
                .then(() => 'error')
                .catch(() => 'timeout');

            // Wait for either message to appear
            const result = await Promise.race([successPromise, errorPromise]);

            if (result === 'success') {
                console.log('✅ Bridge funding transaction successful: Funds sent from faucet!');
                success = true;
            } else if (result === 'error') {
                console.log(
                    `❌ Bridge funding transaction failed on attempt ${attempt}/${maxRetries}, retrying...`,
                );
                // Wait a bit before retrying
                await page.waitForTimeout(3000);
            } else {
                console.log(
                    '⏱️ Bridge funding transaction timed out on attempt ${attempt}/${maxRetries}, retrying...',
                );
                await page.waitForTimeout(3000);
            }
        } catch (error) {
            console.error(`Error during attempt ${attempt}:`, error);
        }

        attempt++;
    }

    if (!success) {
        throw new Error(`Failed to add funds trough bridge UI after ${maxRetries} attempts`);
    }

    // Check the funds arrived (ui)
    const l1WalletExtension = await browser.newPage();
    const l1ExtensionUrl = await getExtensionUrl(browser);
    await l1WalletExtension.goto(l1ExtensionUrl, { waitUntil: 'commit' });
    await expect(l1WalletExtension.getByTestId('coin-balance')).toHaveText('10', {
        timeout: THREE_MINUTES,
    });
    await l1WalletExtension.close();
}
