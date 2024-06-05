// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type Keypair } from '@mysten/sui.js/cryptography';
import { decodeSuiPrivateKey } from '@mysten/sui.js/cryptography/keypair';
import { Ed25519Keypair } from '@mysten/sui.js/keypairs/ed25519';
import { Secp256k1Keypair } from '@mysten/sui.js/keypairs/secp256k1';
import { Secp256r1Keypair } from '@mysten/sui.js/keypairs/secp256r1';

export function fromExportedKeypair(secret: string): Keypair {
    const decoded = decodeSuiPrivateKey(secret);
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
