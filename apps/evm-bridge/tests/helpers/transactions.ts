import { IotaClient, CoinStruct } from '@iota/iota-sdk/client';
import { requestIotaFromFaucet } from '@iota/iota-sdk/faucet';
import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';
import { IOTA_TYPE_ARG, IOTA_DECIMALS, parseAmount } from '@iota/iota-sdk/utils';
import { createDepositTransactionL1 } from '../../src/lib/utils/transaction/createDepositTransactionL1';
import { CONFIG } from '../config/config';
import { TOOL_COIN_OBJECT_ID, TOOL_COIN_TYPE } from '../utils/constants';
import { Transaction } from '@iota/iota-sdk/transactions';
import { bcs } from '@iota/iota-sdk/bcs';
import { Page } from '@playwright/test';

export async function fundL1AddressWithNativeTokens(
    senderAddress: string,
    senderKeypair: Ed25519Keypair,
    addressL1: string,
    amount: number,
) {
    console.log(
        `fundL1AddressWithNativeTokens: Sending ${amount} TOOL from ${senderAddress} to ${addressL1}`,
    );
    try {
        const { L1 } = CONFIG;

        const client = new IotaClient({
            url: L1.rpcUrl,
        });

        const tx = new Transaction();

        const tokenCoin = tx.splitCoins(tx.object(TOOL_COIN_OBJECT_ID), [
            tx.pure(bcs.U64.serialize(amount)),
        ]);
        tx.transferObjects([tokenCoin], addressL1);
        tx.setSender(senderAddress);

        const { digest } = await client.signAndExecuteTransaction({
            signer: senderKeypair,
            transaction: tx,
        });

        await client.waitForTransaction({
            digest,
        });
        console.log(`fundL1AddressWithNativeTokens transaction digest: ${digest}`);
        return true;
    } catch (error) {
        console.error(
            `❌ fundL1AddressWithNativeTokens: Failed to send ${amount} TOOL from ${senderAddress} to ${addressL1}`,
            error,
        );
    }
}

export async function fundL2AddressWithIscClient(
    senderAddress: string,
    senderKeypair: Ed25519Keypair,
    addressL2: string,
    amount: number,
    coinType = IOTA_TYPE_ARG,
) {
    console.log(
        `fundL2AddressWithIscClient: Sending ${amount} ${coinType} from ${senderAddress} to ${addressL2}`,
    );
    try {
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

        let coins: CoinStruct[] = [];

        if (coinType !== IOTA_TYPE_ARG) {
            const { data: toolCoins } = await client.getCoins({
                coinType: TOOL_COIN_TYPE,
                owner: senderAddress,
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
        transaction.setSender(senderAddress);
        await transaction.build({ client });

        const { digest } = await client.signAndExecuteTransaction({
            signer: senderKeypair,
            transaction,
        });
        await client.waitForTransaction({
            digest,
        });
        console.log(`fundL2AddressWithIscClient transaction digest: ${digest}`);
        return true;
    } catch (error) {
        console.error(
            `❌ fundL2AddressWithIscClient: Failed to send ${amount} IOTA from ${senderAddress} to ${addressL2}`,
            error,
        );
        return false;
    }
}

export async function requestFundsFromFaucet(addressL1: string) {
    console.log(`Requesting funds from faucet for ${addressL1}`);
    const { L1 } = CONFIG;
    try {
        await requestIotaFromFaucet({
            host: L1.faucetUrl!,
            recipient: addressL1,
        });

        console.log(`✅ Faucet request successful for ${addressL1}`);
    } catch (error) {
        console.error(`❌ Faucet request failed for ${addressL1}:`, error);
    }
}

export async function addL1FundsThroughBridgeUI(page: Page) {
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
}

export async function sendIotaToAddress(
    senderAddress: string,
    senderKeypair: Ed25519Keypair,
    receiverAddress: string,
    amount: number | string,
) {
    console.log(
        `sendIotaToAddress: Sending ${amount} IOTA from ${senderAddress} to ${receiverAddress}`,
    );

    try {
        const { L1 } = CONFIG;

        const client = new IotaClient({
            url: L1.rpcUrl,
        });

        const amountToSend = parseAmount(amount.toString(), IOTA_DECIMALS) as bigint;

        const transaction = new Transaction();
        const coin = transaction.splitCoins(transaction.gas, [amountToSend]);
        transaction.transferObjects([coin], transaction.pure.address(receiverAddress));

        transaction.setSender(senderAddress);
        await transaction.build({ client });

        const { digest } = await client.signAndExecuteTransaction({
            signer: senderKeypair,
            transaction,
        });

        await client.waitForTransaction({
            digest,
        });

        console.log(`sendIotaToAddress transaction digest: ${digest}`);
    } catch (error) {
        console.error(
            `❌ sendIotaToAddress: Failed to send ${amount} IOTA from ${senderAddress} to ${receiverAddress}`,
            error,
        );
    }
}
