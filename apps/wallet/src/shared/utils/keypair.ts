// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { decodeIotaPrivateKey, type Keypair } from '@iota/iota-sdk/cryptography';
import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';
import { Secp256k1Keypair } from '@iota/iota-sdk/keypairs/secp256k1';
import { Secp256r1Keypair } from '@iota/iota-sdk/keypairs/secp256r1';

export function deriveKeypairFromSeed(seedHex: string, derivationPath: string) {
    return Ed25519Keypair.deriveKeypairFromSeed(seedHex, derivationPath);
}

export function fromExportedKeypair(secret: string): Keypair {
    const decoded = decodeIotaPrivateKey(secret);
    const schema = decoded.schema;
    const secretKey = decoded.secretKey;
    switch (schema) {
        case 'ED25519':
            return Ed25519Keypair.fromSecretKey(secretKey);
        case 'Secp256k1':
            return Secp256k1Keypair.fromSecretKey(secretKey);
        case 'Secp256r1':
            return Secp256r1Keypair.fromSecretKey(secretKey);
        default:
            throw new Error(`Invalid keypair schema ${schema}`);
    }
}
