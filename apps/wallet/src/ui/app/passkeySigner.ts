// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type PasskeyAccountSerializedUI } from '_src/background/accounts/passkeyAccount';
import { type SignedMessage, type SignedTransaction, WalletSigner } from './walletSigner';
import { type IotaClient } from '@iota/iota-sdk/client';
import { fromBase64, toBase64 } from '@iota/iota-sdk/utils';
import { type BrowserPasskeyProvider, PasskeyKeypair } from '@iota/iota-sdk/keypairs/passkey';
import { createBrowserPasskeyProvider } from './helpers/passkeys';

enum SignatureType {
    Transaction = 'transaction',
    Message = 'message',
}

export class PasskeySigner extends WalletSigner {
    readonly #address: string;
    readonly #publicKey: string;
    readonly #provider: BrowserPasskeyProvider;
    readonly #credentialId: Uint8Array;

    constructor(
        { address, providerOptions, publicKey, credentialId }: PasskeyAccountSerializedUI,
        client: IotaClient,
    ) {
        super(client);
        this.#address = address;
        const { provider } = createBrowserPasskeyProvider({ providerOptions });
        this.#provider = provider;
        this.#publicKey = publicKey;
        this.#credentialId = new Uint8Array(credentialId);
    }

    async getAddress(): Promise<string> {
        return this.#address;
    }

    async signMessage(input: { message: Uint8Array }): Promise<SignedMessage> {
        const signature = await this.#requestSignature(input.message, SignatureType.Message);
        return {
            bytes: toBase64(input.message),
            signature,
        };
    }

    async signTransactionBytes(bytes: Uint8Array): Promise<SignedTransaction> {
        const signature = await this.#requestSignature(bytes, SignatureType.Transaction);
        return {
            bytes: toBase64(bytes),
            signature,
        };
    }

    async #requestSignature(data: Uint8Array, signatureType: SignatureType): Promise<string> {
        try {
            const publicKeyBytes = fromBase64(this.#publicKey);
            const keypair = new PasskeyKeypair(publicKeyBytes, this.#provider, this.#credentialId);

            const { signature } =
                signatureType === SignatureType.Transaction
                    ? await keypair.signTransaction(data)
                    : await keypair.signPersonalMessage(data);
            return signature;
        } catch (error) {
            if (error instanceof Error) {
                throw new Error(`Passkey signing failed: ${error.message}`);
            } else {
                throw new Error('Passkey signing failed: Unknown error');
            }
        }
    }
}
