// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { PublicKey } from '../../cryptography/publickey.js';
import { SIGNATURE_SCHEME_TO_FLAG } from '../../cryptography/signature-scheme.js';

/**
 * A MoveAuthenticator public key. Since MoveAuthenticator uses account abstraction,
 * this uses the object ID as the identity rather than a traditional cryptographic public key.
 */
export class MoveAuthenticatorPublicKey extends PublicKey {
    private authenticatedObjectId: Uint8Array;

    /**
     * Creates a new MoveAuthenticatorPublicKey from an account object ID.
     *
     * @param authenticatedObjectId - The object ID as bytes (32 bytes)
     */
    constructor(authenticatedObjectId: Uint8Array | string) {
        super();
        if (typeof authenticatedObjectId === 'string') {
            // Convert hex string to bytes (remove 0x prefix if present)
            const hex = authenticatedObjectId.startsWith('0x')
                ? authenticatedObjectId.slice(2)
                : authenticatedObjectId;
            this.authenticatedObjectId = new Uint8Array(
                hex.match(/.{1,2}/g)?.map((byte) => parseInt(byte, 16)) || [],
            );
        } else {
            this.authenticatedObjectId = authenticatedObjectId;
        }
    }

    /**
     * Return the byte array representation of the object ID
     */
    toRawBytes(): Uint8Array {
        return this.authenticatedObjectId;
    }

    /**
     * Return the signature scheme flag for MoveAuthenticator
     */
    flag(): number {
        return SIGNATURE_SCHEME_TO_FLAG.MoveAuthenticator;
    }

    /**
     * Verification is not supported for MoveAuthenticator as it uses account abstraction.
     * The verification happens on-chain via the authenticator function.
     */
    async verify(_data: Uint8Array, _signature: Uint8Array | string): Promise<boolean> {
        throw new Error(
            'Verification is not supported for MoveAuthenticator. Verification happens on-chain via the authenticator function.',
        );
    }
}
