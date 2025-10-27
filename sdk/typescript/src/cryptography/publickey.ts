// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { toBase64 } from '@iota/bcs';
import { blake2b } from '@noble/hashes/blake2b';
import { bytesToHex } from '@noble/hashes/utils';

import { bcs } from '../bcs/index.js';
import { normalizeIotaAddress, IOTA_ADDRESS_LENGTH } from '../utils/iota-types.js';
import type { IntentScope } from './intent.js';
import { messageWithIntent } from './intent.js';
import { SIGNATURE_SCHEME_TO_FLAG } from './signature-scheme.js';

/**
 * Value to be converted into public key.
 */
export type PublicKeyInitData = string | Uint8Array | Iterable<number>;

export function bytesEqual(a: Uint8Array, b: Uint8Array) {
    if (a === b) return true;

    if (a.length !== b.length) {
        return false;
    }

    for (let i = 0; i < a.length; i++) {
        if (a[i] !== b[i]) {
            return false;
        }
    }
    return true;
}

/**
 * A public key
 */
export abstract class PublicKey {
    /**
     * Checks if two public keys are equal
     */
    equals(publicKey: PublicKey) {
        return bytesEqual(this.toRawBytes(), publicKey.toRawBytes());
    }

    /**
     * Return the base-64 representation of the public key
     */
    toBase64() {
        return toBase64(this.toRawBytes());
    }

    toString(): never {
        throw new Error(
            '`toString` is not implemented on public keys. Use `toBase64()` or `toRawBytes()` instead.',
        );
    }

    /**
     * Return the IOTA representation of the public key encoded in
     * base-64. An IOTA public key is formed by the concatenation
     * of the scheme flag with the raw bytes of the public key
     */
    toIotaPublicKey(): string {
        const bytes = this.toIotaBytes();
        return toBase64(bytes);
    }

    verifyWithIntent(
        bytes: Uint8Array,
        signature: Uint8Array | string,
        intent: IntentScope,
    ): Promise<boolean> {
        const intentMessage = messageWithIntent(intent, bytes);
        const digest = blake2b(intentMessage, { dkLen: 32 });

        return this.verify(digest, signature);
    }

    /**
     * Verifies that the signature is valid for the provided PersonalMessage
     */
    verifyPersonalMessage(message: Uint8Array, signature: Uint8Array | string): Promise<boolean> {
        return this.verifyWithIntent(
            bcs.vector(bcs.u8()).serialize(message).toBytes(),
            signature,
            'PersonalMessage',
        );
    }

    /**
     * Verifies that the signature is valid for the provided Transaction
     */
    verifyTransaction(transaction: Uint8Array, signature: Uint8Array | string): Promise<boolean> {
        return this.verifyWithIntent(transaction, signature, 'TransactionData');
    }

    /**
     * Returns the bytes representation of the public key
     * prefixed with the signature scheme flag
     */
    toIotaBytes(): Uint8Array {
        const rawBytes = this.toRawBytes();
        const iotaBytes = new Uint8Array(rawBytes.length + 1);
        iotaBytes.set([this.flag()]);
        iotaBytes.set(rawBytes, 1);

        return iotaBytes;
    }

    /**
     * Returns the bytes representation of the public key
     * prefixed with the signature scheme flag. If the
     * signature scheme is ED25519, no prefix is set.
     */
    toIotaBytesForAddress(): Uint8Array {
        const rawBytes = this.toRawBytes();
        if (this.flag() === SIGNATURE_SCHEME_TO_FLAG['ED25519']) {
            return rawBytes;
        } else {
            const iotaBytes = new Uint8Array(rawBytes.length + 1);
            iotaBytes.set([this.flag()]);
            iotaBytes.set(rawBytes, 1);

            return iotaBytes;
        }
    }

    /**
     * Return the IOTA address associated with this Ed25519 public key
     */
    toIotaAddress(): string {
        // Each hex char represents half a byte, hence hex address doubles the length
        return normalizeIotaAddress(
            bytesToHex(blake2b(this.toIotaBytesForAddress(), { dkLen: 32 })).slice(
                0,
                IOTA_ADDRESS_LENGTH * 2,
            ),
        );
    }

    /**
     * Return the byte array representation of the public key
     */
    abstract toRawBytes(): Uint8Array;

    /**
     * Return signature scheme flag of the public key
     */
    abstract flag(): number;

    /**
     * Verifies that the signature is valid for the provided message
     */
    abstract verify(data: Uint8Array, signature: Uint8Array | string): Promise<boolean>;
}
