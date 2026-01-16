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
        const authenticatedObjectId =
            this.data.objectToAuthenticate.$kind === 'Immutable'
                ? this.data.objectToAuthenticate.Immutable.objectId
                : this.data.objectToAuthenticate.Shared.objectId;
        return new MoveAuthenticatorPublicKey(authenticatedObjectId);
    }

    /**
     * Serialize the MoveAuthenticator data to bytes (without the signature scheme flag).
     * The bytes parameter is ignored for MoveAuthenticator as we don't perform traditional signing.
     *
     * @param _bytes - Ignored for MoveAuthenticator
     * @returns The BCS-serialized MoveAuthenticator bytes
     */
    async sign(_bytes: Uint8Array): Promise<Uint8Array> {
        // Convert the data to BCS format
        const bcsCallArgs = this.data.callArgs.map((arg) => {
            if (arg.$kind === 'ImmutableOrOwned') {
                return {
                    ImmutableOrOwned: {
                        objectId: arg.ImmutableOrOwned.objectId,
                        version: arg.ImmutableOrOwned.version,
                        digest: arg.ImmutableOrOwned.digest,
                    },
                };
            } else if (arg.$kind === 'Shared') {
                return {
                    Shared: {
                        objectId: arg.Shared.objectId,
                        initialSharedVersion: arg.Shared.initialSharedVersion,
                        mutable: arg.Shared.mutable,
                    },
                };
            } else {
                return {
                    Pure: arg.Pure,
                };
            }
        });

        const bcsAccount =
            this.data.objectToAuthenticate.$kind === 'Immutable'
                ? {
                      Immutable: {
                          objectId: this.data.objectToAuthenticate.Immutable.objectId,
                          version: this.data.objectToAuthenticate.Immutable.version,
                          digest: this.data.objectToAuthenticate.Immutable.digest,
                      },
                  }
                : {
                      Shared: {
                          objectId: this.data.objectToAuthenticate.Shared.objectId,
                          initialSharedVersion:
                              this.data.objectToAuthenticate.Shared.initialSharedVersion,
                      },
                  };

        return bcs.MoveAuthenticator.serialize({
            callArgs: bcsCallArgs,
            typeArgs: this.data.typeArgs,
            account: bcsAccount,
        }).toBytes();
    }

    /**
     * Override signWithIntent to handle MoveAuthenticator's special serialization format.
     * Unlike traditional signatures, MoveAuthenticator doesn't follow the standard
     * flag || signature || publicKey format.
     */
    async signWithIntent(bytes: Uint8Array, _intent: IntentScope): Promise<SignatureWithBytes> {
        // For MoveAuthenticator, we serialize the authenticator data
        // The transaction bytes and intent are not directly used in the serialization
        const serialized = await this.sign(bytes);

        // Prepend the MoveAuthenticator flag
        const result = new Uint8Array(1 + serialized.length);
        result[0] = SIGNATURE_SCHEME_TO_FLAG[this.getKeyScheme()];
        result.set(serialized, 1);

        return {
            signature: toBase64(result),
            bytes: toBase64(bytes),
        };
    }
}
