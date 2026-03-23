// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { IotaClient } from '@iota/iota-sdk/client';
import { requestIotaFromFaucet } from '@iota/iota-sdk/faucet';
import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';
import { Transaction } from '@iota/iota-sdk/transactions';
import type { AssetsResponse } from '../src/index.js';
import { EvmRpcClient } from '../src/index.js';
import { CONFIG } from './config.js';
import { NANOS_PER_IOTA } from '@iota/iota-sdk/utils';

const { L2 } = CONFIG;

export async function requestFunds(
    client: IotaClient,
    faucetUrl: string,
    recipientAddress: string,
) {
    const keypair = new Ed25519Keypair();
    const address = keypair.toIotaAddress();

    await requestIotaFromFaucet({
        host: faucetUrl,
        recipient: address,
    });

    const transaction = new Transaction();
    const [coin] = transaction.splitCoins(transaction.gas, [1n * NANOS_PER_IOTA]);
    transaction.transferObjects([coin], recipientAddress);
    transaction.setSender(address);

    await transaction.build({ client });

    await client.signAndExecuteTransaction({
        signer: keypair,
        transaction,
    });
}

export async function checkL2BalanceWithRetries(
    address: string,
    coinType?: string,
    maxRetries = 10,
    delay = 2500,
): Promise<AssetsResponse | null> {
    const evmClient = new EvmRpcClient(L2.evmRpcUrl);
    let evmBalance: AssetsResponse | null = null;

    for (let attempt = 1; attempt <= maxRetries; attempt++) {
        try {
            evmBalance = await evmClient.getBalanceBaseToken(address);
        } catch (error) {
            console.error('Error checking balance:', error);
        } finally {
            const nativeToken = evmBalance?.nativeTokens?.find((t) => t.coinType === coinType);
            const nativeTokenBalance = nativeToken ? nativeToken.balance : '0';

            if (
                (evmBalance?.baseTokens.startsWith('0') ||
                    (coinType && nativeTokenBalance.startsWith('0'))) &&
                attempt < maxRetries
            ) {
                console.log(
                    `Fetching EVM balance attempt ${attempt + 1} out of ${maxRetries} in ${delay} ms`,
                );
                await new Promise((resolve) => setTimeout(resolve, delay));
            }
        }
    }

    return evmBalance;
}
