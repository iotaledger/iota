// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import 'tsconfig-paths/register';

import { IotaClient, getFullnodeUrl } from '@iota/iota-sdk/client';
import { type Keypair } from '@iota/iota-sdk/cryptography';
import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';
import { Transaction } from '@iota/iota-sdk/transactions';

const addressToKeypair = new Map<string, Keypair>();

export async function split_coin(address: string) {
    const keypair = addressToKeypair.get(address);
    if (!keypair) {
        throw new Error('missing keypair');
    }
    const client = new IotaClient({ url: getFullnodeUrl('localnet') });

    const coins = await client.getCoins({ owner: address });
    const coin_id = coins.data[0].coinObjectId;

    const tx = new Transaction();
    tx.moveCall({
        target: '0x2::pay::split',
        typeArguments: ['0x2::iota::IOTA'],
        arguments: [tx.object(coin_id), tx.pure.u64(10)],
    });

    const result = await client.signAndExecuteTransaction({
        signer: keypair,
        transaction: tx,
        options: {
            showInput: true,
            showEffects: true,
            showEvents: true,
        },
    });

    await client.waitForTransaction({ digest: result.digest });

    return result;
}

export async function faucet() {
    const keypair = Ed25519Keypair.generate();
    const address = keypair.getPublicKey().toIotaAddress();
    addressToKeypair.set(address, keypair);
    const res = await fetch('http://127.0.0.1:9123/gas', {
        method: 'POST',
        headers: {
            'content-type': 'application/json',
        },
        body: JSON.stringify({ FixedAmountRequest: { recipient: address } }),
    });
    const data = await res.json();
    if (!res.ok || data.error) {
        throw new Error('Unable to invoke local faucet.');
    }
    return address;
}
