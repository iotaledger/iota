// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ed25519 } from '@noble/curves/ed25519';

import { Signer } from '@iota/iota-sdk/cryptography';
import type { SignatureScheme } from '@iota/iota-sdk/cryptography';
import { Ed25519PublicKey } from '@iota/iota-sdk/keypairs/ed25519';
import { Transaction } from '@iota/iota-sdk/transactions';
import { getFullnodeUrl, IotaClient } from '@iota/iota-sdk/client';
import { getFaucetHost, requestIotaFromFaucetV1 } from '@iota/iota-sdk/faucet';

/**
 * Example: Custom Signer
 *
 * This example demonstrates how to build the most basic custom signer by
 * extending the abstract `Signer` class from the IOTA SDK. You only need to
 * implement three methods: `sign`, `getKeyScheme`, and `getPublicKey`.
 *
 * The base `Signer` class then provides `signTransaction`,
 * `signPersonalMessage`, and `toIotaAddress` for free.
 */

class MyEd25519Signer extends Signer {
    private secretKey: Uint8Array;
    private publicKey: Ed25519PublicKey;

    constructor(secretKey: Uint8Array) {
        super();
        this.secretKey = secretKey;
        this.publicKey = new Ed25519PublicKey(ed25519.getPublicKey(secretKey));
    }

    getKeyScheme(): SignatureScheme {
        return 'ED25519';
    }

    getPublicKey(): Ed25519PublicKey {
        return this.publicKey;
    }

    async sign(bytes: Uint8Array): Promise<Uint8Array> {
        return ed25519.sign(bytes, this.secretKey);
    }
}

// Create a client connected to devnet
const client = new IotaClient({ url: getFullnodeUrl('devnet') });

// Generate a random 32-byte secret key (in a real app this would come from
// your own key-management system, HSM, KMS, etc.)
const secretKey = ed25519.utils.randomPrivateKey();
const signer = new MyEd25519Signer(secretKey);

console.log('Custom signer address:', signer.toIotaAddress());

// Sign a personal message
const message = new TextEncoder().encode('Hello IOTA from a custom signer!');
const { signature: personalSig } = await signer.signPersonalMessage(message);
console.log('Personal message signature:', personalSig);

// Request IOTA from devnet faucet
await requestIotaFromFaucetV1({
    host: getFaucetHost('devnet'),
    recipient: signer.toIotaAddress(),
});

// Build a transaction and sign it (without executing)
const tx = new Transaction();
const [coin] = tx.splitCoins(tx.gas, [1_000]);
tx.transferObjects([coin], signer.toIotaAddress());
tx.setSender(signer.toIotaAddress());

const { signature: txSig, bytes: txBytes } = await tx.sign({ signer, client });
console.log('Transaction bytes (base64):', txBytes);
console.log('Transaction signature:', txSig);
