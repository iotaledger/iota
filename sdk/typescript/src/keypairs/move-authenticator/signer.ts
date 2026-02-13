// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { toBase64 } from '@iota/bcs';

import { bcs } from '../../bcs/index.js';
import type { IntentScope, SignatureWithBytes } from '../../cryptography/index.js';
import { SIGNATURE_SCHEME_TO_FLAG, Signer } from '../../cryptography/index.js';
import type { PublicKey } from '../../cryptography/publickey.js';
import type { SignatureScheme } from '../../cryptography/signature-scheme.js';
import { MoveAuthenticatorPublicKey } from './publickey.js';
import type { MoveAuthenticatorData } from './types.js';
import type { ObjectArg } from '../../bcs/types.js';

/**
 * Extract the object ID from a resolved CallArg that represents the object to authenticate.
 */
function getObjectIdFromCallArg(callArg: ObjectArg): string {
    if ('ImmOrOwnedObject' in callArg) {
        return callArg.ImmOrOwnedObject.objectId;
    } else if ('SharedObject' in callArg) {
        return callArg.SharedObject.objectId;
    } else {
        return callArg.Receiving.objectId;
    }
}

/**
 * A Move Authenticator signer for account abstraction.
 * This allows transactions to be authorized via Move functions rather than traditional cryptographic signatures.
 */
export class MoveSigner extends Signer {
    private data: MoveAuthenticatorData;

    /**
     * Creates a new MoveSigner with the provided MoveAuthenticator data.
     *
     * @param data - The MoveAuthenticator data containing object to authenticate, call arguments, and type arguments
     */
    constructor(data: MoveAuthenticatorData) {
        super();
        this.data = data;
    }

    /**
     * Get the key scheme for MoveAuthenticator
     */
    getKeyScheme(): SignatureScheme {
        return 'MoveAuthenticator';
    }

    /**
     * Return the public key for this MoveAuthenticator.
     * Since MoveAuthenticator uses account abstraction, this returns a public key
     * based on the object ID.
     */
    getPublicKey(): PublicKey {
        const objectId = getObjectIdFromCallArg(this.data.objectToAuthenticate);
        return new MoveAuthenticatorPublicKey(objectId);
    }

    /**
     * Serialize the MoveAuthenticator data to bytes (without the signature scheme flag).
     * The data is already in the correct CallArg BCS format.
     *
     * @returns The BCS-serialized MoveAuthenticator bytes
     */
    async sign(): Promise<Uint8Array> {
        return bcs.MoveAuthenticator.serialize({
            callArgs: this.data.callArgs,
            typeArgs: this.data.typeArgs,
            objectToAuthenticate: {
                Object: this.data.objectToAuthenticate,
            },
        }).toBytes();
    }

    /**
     * Override signWithIntent to handle MoveAuthenticator's special serialization format.
     * Unlike traditional signatures, MoveAuthenticator doesn't follow the standard
     * `flag || signature || publicKey` format.
     * It follows `flag || signature` format.
     */
    async signWithIntent(bytes: Uint8Array, _intent: IntentScope): Promise<SignatureWithBytes> {
        const signature = await this.getSignature();

        return {
            signature,
            bytes: toBase64(bytes),
        };
    }

    /**
     * Generates the Move Authenticator signature.
     */
    async getSignature(): Promise<string> {
        const serialized = await this.sign();

        // Prepend the MoveAuthenticator flag
        const result = new Uint8Array(1 + serialized.length);
        result[0] = SIGNATURE_SCHEME_TO_FLAG[this.getKeyScheme()];
        result.set(serialized, 1);
        return toBase64(result);
    }
}
