// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { fromHex } from '@iota/bcs';
import { PublicKey } from '../../cryptography/publickey.js';
import { SIGNATURE_SCHEME_TO_FLAG } from '../../cryptography/signature-scheme.js';
import { normalizeIotaAddress } from '../../utils/iota-types.js';
import { bytesToHex } from '@noble/hashes/utils';

/**
 * @experimental
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
            this.authenticatedObjectId = fromHex(authenticatedObjectId);
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
     * Return the IOTA address for this MoveAuthenticator.
     * Unlike other key types, the address IS the object ID directly,
     * not a hash of (flag || publicKey). This matches the Rust implementation:
     * `IotaAddress::from(object_id)`.
     */
    override toIotaAddress(): string {
        return normalizeIotaAddress(bytesToHex(this.authenticatedObjectId));
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
