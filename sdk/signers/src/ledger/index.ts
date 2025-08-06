// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type IotaLedgerClient from '@iota/ledgerjs-hw-app-iota';
import type { IotaClient } from '@iota/iota-sdk/client';
import type { SignatureWithBytes } from '@iota/iota-sdk/cryptography';
import { messageWithIntent, Signer, toSerializedSignature } from '@iota/iota-sdk/cryptography';
import { Ed25519PublicKey } from '@iota/iota-sdk/keypairs/ed25519';
import { Transaction } from '@iota/iota-sdk/transactions';
import { toB64 } from '@iota/iota-sdk/utils';

import { IotaMoveObject } from './bcs.js';
import { bcs } from '@iota/iota-sdk/bcs';

/**
 * Configuration options for initializing the LedgerSigner.
 */
export interface LedgerSignerOptions {
    publicKey: Ed25519PublicKey;
    derivationPath: string;
    ledgerClient: IotaLedgerClient;
    iotaClient: IotaClient;
}

/**
 * Ledger integrates with the Iota blockchain to provide signing capabilities using Ledger devices.
 */
export class LedgerSigner extends Signer {
    #derivationPath: string;
    #publicKey: Ed25519PublicKey;
    #ledgerClient: IotaLedgerClient;
    #iotaClient: IotaClient;

    /**
     * Creates an instance of LedgerSigner. It's expected to call the static `fromDerivationPath` method to create an instance.
     * @example
     * ```
     * const signer = await LedgerSigner.fromDerivationPath(derivationPath, options);
     * ```
     */
    constructor({ publicKey, derivationPath, ledgerClient, iotaClient }: LedgerSignerOptions) {
        super();
        this.#publicKey = publicKey;
        this.#derivationPath = derivationPath;
        this.#ledgerClient = ledgerClient;
        this.#iotaClient = iotaClient;
    }

    /**
     * Retrieves the key scheme used by this signer.
     */
    override getKeyScheme() {
        return 'ED25519' as const;
    }

    /**
     * Retrieves the public key associated with this signer.
     * @returns The Ed25519PublicKey instance.
     */
    override getPublicKey() {
        return this.#publicKey;
    }

    /**
     * Signs the provided transaction bytes.
     * @returns The signed transaction bytes and signature.
     */
    override async signTransaction(bytes: Uint8Array): Promise<SignatureWithBytes> {
        const transactionOptions = await this.#getClearSigningOptions(bytes).catch(() => ({
            // Fail gracefully so network errors or serialization issues don't break transaction signing:
            bcsObjects: [],
        }));

        const intentMessage = messageWithIntent('TransactionData', bytes);
        const { signature } = await this.#ledgerClient.signTransaction(
            this.#derivationPath,
            intentMessage,
            transactionOptions,
        );

        return {
            bytes: toB64(bytes),
            signature: toSerializedSignature({
                signature,
                signatureScheme: this.getKeyScheme(),
                publicKey: this.#publicKey,
            }),
        };
    }

    /**
     * Signs the provided personal message.
     * @returns The signed message bytes and signature.
     */
    override async signPersonalMessage(bytes: Uint8Array): Promise<SignatureWithBytes> {
        const intentMessage = messageWithIntent(
            'PersonalMessage',
            bcs.byteVector().serialize(bytes).toBytes(),
        );
        const { signature } = await this.#ledgerClient.signTransaction(
            this.#derivationPath,
            intentMessage,
        );

        return {
            bytes: toB64(bytes),
            signature: toSerializedSignature({
                signature,
                signatureScheme: this.getKeyScheme(),
                publicKey: this.#publicKey,
            }),
        };
    }

    /**
     * Prepares the signer by fetching and setting the public key from a Ledger device.
     * It is recommended to initialize an `LedgerSigner` instance using this function.
     * @returns A promise that resolves once a `LedgerSigner` instance is prepared (public key is set).
     */
    static async fromDerivationPath(
        derivationPath: string,
        ledgerClient: IotaLedgerClient,
        iotaClient: IotaClient,
    ) {
        const { publicKey } = await ledgerClient.getPublicKey(derivationPath);
        if (!publicKey) {
            throw new Error('Failed to get public key from Ledger.');
        }

        return new LedgerSigner({
            derivationPath,
            publicKey: new Ed25519PublicKey(publicKey),
            ledgerClient,
            iotaClient,
        });
    }

    async #getClearSigningOptions(transactionBytes: Uint8Array) {
        const transaction = Transaction.from(transactionBytes);
        const data = transaction.getData();

        const gasObjectIds = data.gasData.payment?.map((object) => object.objectId) ?? [];
        const inputObjectIds = data.inputs
            .map((input) => {
                return input.$kind === 'Object' && input.Object.$kind === 'ImmOrOwnedObject'
                    ? input.Object.ImmOrOwnedObject.objectId
                    : null;
            })
            .filter((objectId): objectId is string => !!objectId);

        const objects = await this.#iotaClient.multiGetObjects({
            ids: [...gasObjectIds, ...inputObjectIds],
            options: {
                showBcs: true,
                showPreviousTransaction: true,
                showStorageRebate: true,
                showOwner: true,
            },
        });

        // NOTE: We should probably get rid of this manual serialization logic in favor of using the
        // already serialized object bytes from the GraphQL API once there is more mainstream support
        // for it + we can enforce the transport type on the Iota client.
        const bcsObjects = objects
            .map((object) => {
                if (object.error || !object.data || object.data.bcs?.dataType !== 'moveObject') {
                    return null;
                }

                return IotaMoveObject.serialize({
                    data: {
                        MoveObject: {
                            type: object.data.bcs.type,
                            version: object.data.bcs.version,
                            contents: object.data.bcs.bcsBytes,
                        },
                    },
                    owner: object.data.owner!,
                    previousTransaction: object.data.previousTransaction!,
                    storageRebate: object.data.storageRebate!,
                }).toBytes();
            })
            .filter((bcsBytes): bcsBytes is Uint8Array => !!bcsBytes);

        return { bcsObjects };
    }

    /**
     * Generic signing is not supported by Ledger.
     * @throws Always throws an error indicating generic signing is unsupported.
     */
    override sign(): never {
        throw new Error('Ledger Signer does not support generic signing.');
    }

    /**
     * Generic signing is not supported by Ledger.
     * @throws Always throws an error indicating generic signing is unsupported.
     */
    override signWithIntent(): never {
        throw new Error('Ledger Signer does not support generic signing.');
    }
}
