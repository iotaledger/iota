// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { CallArg, ObjectArg } from '../../bcs/types.js';
import type { IotaClient } from '../../client/index.js';
import type { MoveAuthenticatorCallArg, MoveAuthenticatorData } from './types.js';

/**
 * Error thrown when an invalid argument is provided to MoveAuthenticator.
 */
export class InvalidMoveAuthArgError extends Error {
    constructor(message: string) {
        super(message);
        this.name = 'InvalidMoveAuthArgError';
    }
}

/**
 * Error thrown when an invalid objectToAuthenticate is provided to MoveAuthenticator.
 */
export class InvalidMoveAuthAccountError extends Error {
    constructor(message: string) {
        super(message);
        this.name = 'InvalidMoveAuthAccountError';
    }
}

/**
 * A function call to authorize a transaction via Move.
 * This builder creates a MoveAuthenticator which can be used to execute
 * a transaction with Account Abstraction.
 */
export class MoveAuthenticatorBuilder {
    private callArgs: MoveAuthenticatorCallArg[] = [];
    private typeArgs: string[] = [];
    private objectToAuthenticate: string;

    /**
     * Create a new Move Authenticator builder from an object ID, which is the
     * sender of a transaction that this will be used to authenticate.
     *
     * @param objectToAuthenticate - The object ID of the objectToAuthenticate (sender of the transaction)
     */
    constructor(objectToAuthenticate: string) {
        this.objectToAuthenticate = objectToAuthenticate;
    }

    /**
     * Set the move authenticator call inputs.
     * Call arguments must not be owned objects - they must be immutable or shared.
     *
     * @param args - Array of input kinds specifying the call arguments
     * @returns this builder for chaining
     */
    setCallArgs(args: MoveAuthenticatorCallArg[]): this {
        this.callArgs = args;
        return this;
    }

    /**
     * Add a single call argument.
     *
     * @param arg - The input kind to add
     * @returns this builder for chaining
     */
    addCallArg(arg: MoveAuthenticatorCallArg): this {
        this.callArgs.push(arg);
        return this;
    }

    /**
     * Add an immutable or owned object as a call argument.
     * Note: The object must actually be immutable when resolved.
     *
     * @param objectId - The object ID
     * @returns this builder for chaining
     */
    addImmutableObject(objectId: string): this {
        this.callArgs.push({ ImmutableOrOwned: objectId });
        return this;
    }

    /**
     * Add a shared object as a call argument.
     *
     * @param objectId - The object ID
     * @param mutable - Whether the object is accessed mutably
     * @returns this builder for chaining
     */
    addSharedObject(objectId: string, mutable: boolean = false): this {
        this.callArgs.push({ Shared: { objectId, mutable } });
        return this;
    }

    /**
     * Add a pure value as a call argument.
     *
     * @param value - The pure value bytes
     * @returns this builder for chaining
     */
    addPure(value: Uint8Array): this {
        this.callArgs.push({ Pure: value });
        return this;
    }

    /**
     * Set the move authenticator call type parameters.
     *
     * @param typeArgs - Array of type argument strings
     * @returns this builder for chaining
     */
    setTypeArgs(typeArgs: string[]): this {
        this.typeArgs = typeArgs;
        return this;
    }

    /**
     * Add a single type argument.
     *
     * @param typeArg - The type argument string
     * @returns this builder for chaining
     */
    addTypeArg(typeArg: string): this {
        this.typeArgs.push(typeArg);
        return this;
    }

    /**
     * Resolve this move authenticator builder into a MoveAuthenticator
     * which can be used to execute the given transaction.
     *
     * @param client - The IOTA client to use for fetching object data
     * @returns The resolved MoveAuthenticator data
     * @throws InvalidMoveAuthArgError if call arguments are invalid
     * @throws InvalidMoveAuthAccountError if the object is invalid
     */
    async finish(client: IotaClient): Promise<MoveAuthenticatorData> {
        // Fetch the object
        const objectToAuthenticateResponse = await client.getObject({
            id: this.objectToAuthenticate,
            options: { showOwner: true },
        });

        if (!objectToAuthenticateResponse.data) {
            throw new InvalidMoveAuthArgError(
                `missing objectToAuthenticate ${this.objectToAuthenticate}`,
            );
        }

        const objectToAuthenticateData = objectToAuthenticateResponse.data;
        const objectToAuthenticateOwner = objectToAuthenticateData.owner;

        // Resolve call arguments
        const resolvedCallArgs: CallArg[] = [];

        for (const input of this.callArgs) {
            if ('ImmutableOrOwned' in input) {
                const objectId = input.ImmutableOrOwned;
                const objResponse = await client.getObject({
                    id: objectId,
                    options: { showOwner: true },
                });

                if (!objResponse.data) {
                    throw new InvalidMoveAuthArgError(`missing object ${objectId}`);
                }

                const objData = objResponse.data;
                const owner = objData.owner;

                // Check if the object is immutable
                if (owner !== 'Immutable') {
                    throw new InvalidMoveAuthArgError('call arguments must not be owned');
                }

                resolvedCallArgs.push({
                    Object: {
                        ImmOrOwnedObject: {
                            objectId: objData.objectId,
                            version: objData.version!,
                            digest: objData.digest!,
                        },
                    },
                });
            } else if ('Shared' in input) {
                const { objectId, mutable } = input.Shared;
                const objResponse = await client.getObject({
                    id: objectId,
                    options: { showOwner: true },
                });

                if (!objResponse.data) {
                    throw new InvalidMoveAuthArgError(`missing object ${objectId}`);
                }

                const objData = objResponse.data;
                const owner = objData.owner;

                // Check if the object is shared
                if (!owner || typeof owner !== 'object' || !('Shared' in owner)) {
                    throw new InvalidMoveAuthArgError(
                        `object ${objectId} was passed as shared, but is not`,
                    );
                }

                resolvedCallArgs.push({
                    Object: {
                        SharedObject: {
                            objectId,
                            initialSharedVersion: owner.Shared.initial_shared_version,
                            mutable,
                        },
                    },
                });
            } else if ('Pure' in input) {
                resolvedCallArgs.push({
                    Pure: {
                        bytes: input.Pure,
                    },
                });
            }
        }

        // Resolve objectToAuthenticate
        let objectToAuthenticate: ObjectArg;

        if (objectToAuthenticateOwner === 'Immutable') {
            objectToAuthenticate = {
                ImmOrOwnedObject: {
                    objectId: objectToAuthenticateData.objectId,
                    version: objectToAuthenticateData.version!,
                    digest: objectToAuthenticateData.digest!,
                },
            };
        } else if (
            objectToAuthenticateOwner &&
            typeof objectToAuthenticateOwner === 'object' &&
            'Shared' in objectToAuthenticateOwner
        ) {
            objectToAuthenticate = {
                SharedObject: {
                    objectId: objectToAuthenticateData.objectId,
                    initialSharedVersion: objectToAuthenticateOwner.Shared.initial_shared_version,
                    mutable: false,
                },
            };
        } else {
            throw new InvalidMoveAuthAccountError(
                'objectToAuthenticate must be immutable or shared',
            );
        }

        return {
            callArgs: resolvedCallArgs,
            typeArgs: this.typeArgs,
            objectToAuthenticate,
        };
    }
}
