// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { SignatureScheme } from '@iota/iota-sdk/cryptography';
import { Signer } from '@iota/iota-sdk/cryptography';
import { Secp256r1PublicKey } from '@iota/iota-sdk/keypairs/secp256r1';
import { secp256r1 } from '@noble/curves/p256';

// Convert from uncompressed (65 bytes) to compressed (33 bytes) format
function getCompressedPublicKey(publicKey: Uint8Array) {
    const rawBytes = new Uint8Array(publicKey);
    const x = rawBytes.slice(1, 33);
    const y = rawBytes.slice(33, 65);

    const prefix = (y[31] & 1) === 0 ? 0x02 : 0x03;

    const compressed = new Uint8Array(Secp256r1PublicKey.SIZE);
    compressed[0] = prefix;
    compressed.set(x, 1);

    return compressed;
}

export interface ExportedWebCryptoKeypair {
    privateKey: CryptoKey;
    publicKey: Uint8Array;
}

export class WebCryptoSigner extends Signer {
    privateKey: CryptoKey;

    #publicKey: Secp256r1PublicKey;

    static async generate({ extractable = false }: { extractable?: boolean } = {}) {
        const keypair = await globalThis.crypto.subtle.generateKey(
            {
                name: 'ECDSA',
                namedCurve: 'P-256',
            },
            extractable,
            ['sign', 'verify'],
        );

        const publicKey = await globalThis.crypto.subtle.exportKey('raw', keypair.publicKey);

        return new WebCryptoSigner(
            keypair.privateKey,
            getCompressedPublicKey(new Uint8Array(publicKey)),
        );
    }

    /**
     * Imports a keypair using the value returned by `export()`.
     */
    static import(data: ExportedWebCryptoKeypair) {
        return new WebCryptoSigner(data.privateKey, data.publicKey);
    }

    getKeyScheme(): SignatureScheme {
        return 'Secp256r1';
    }

    constructor(privateKey: CryptoKey, publicKey: Uint8Array) {
        super();
        this.privateKey = privateKey;
        this.#publicKey = new Secp256r1PublicKey(publicKey);
    }

    /**
     * Exports the keypair so that it can be stored in IndexedDB.
     */
    export(): ExportedWebCryptoKeypair {
        const exportedKeypair = {
            privateKey: this.privateKey,
            publicKey: this.#publicKey.toRawBytes(),
        };

        Object.defineProperty(exportedKeypair, 'toJSON', {
            enumerable: false,
            value: () => {
                throw new Error(
                    'The exported keypair must not be serialized. It must be stored in IndexedDB directly.',
                );
            },
        });

        return exportedKeypair;
    }

    getPublicKey() {
        return this.#publicKey;
    }

    async sign(bytes: Uint8Array): Promise<Uint8Array> {
        const rawSignature = await globalThis.crypto.subtle.sign(
            {
                name: 'ECDSA',
                hash: 'SHA-256',
            },
            this.privateKey,
            bytes,
        );

        const signature = secp256r1.Signature.fromCompact(new Uint8Array(rawSignature));

        return signature.normalizeS().toCompactRawBytes();
    }
}
