// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type IotaLedgerClient from '@iota/ledgerjs-hw-app-iota';
import { type IotaClient } from '@iota/iota-sdk/client';
import { type Ed25519PublicKey } from '@iota/iota-sdk/keypairs/ed25519';
import { LedgerSigner as SignersLedgerSigner } from '@iota/signers/ledger';

import { type SignedMessage, type SignedTransaction, WalletSigner } from './walletSigner';

export class LedgerSigner extends WalletSigner {
    #iotaLedgerClient: IotaLedgerClient | null;
    #signer: SignersLedgerSigner | null = null;
    readonly #connectToLedger: (
        requestPermissionsFirst?: boolean,
        verifyConnection?: (client: IotaLedgerClient) => Promise<boolean>,
    ) => Promise<IotaLedgerClient>;
    readonly #derivationPath: string;
    readonly #expectedAddress: string;

    constructor(
        connectToLedger: (
            requestPermissionsFirst?: boolean,
            verifyConnection?: (client: IotaLedgerClient) => Promise<boolean>,
        ) => Promise<IotaLedgerClient>,
        derivationPath: string,
        expectedAddress: string,
        client: IotaClient,
    ) {
        super(client);
        this.#connectToLedger = connectToLedger;
        this.#iotaLedgerClient = null;
        this.#derivationPath = derivationPath;
        this.#expectedAddress = expectedAddress;
    }

    async #initializeIotaLedgerClient() {
        if (!this.#iotaLedgerClient) {
            // We want to make sure that there's only one connection established per Ledger signer
            // instance since some methods make multiple calls like getAddress and signData
            this.#iotaLedgerClient = await this.#connectToLedger(
                false,
                async (client: IotaLedgerClient) => {
                    try {
                        // Verify device produces the expected address
                        const tempSigner = await SignersLedgerSigner.fromDerivationPath(
                            this.#derivationPath,
                            client,
                            this.client,
                        );
                        const actualAddress = await tempSigner.toIotaAddress();
                        return actualAddress === this.#expectedAddress;
                    } catch (error) {
                        // If verification fails, return false to trigger device selection
                        return false;
                    }
                },
            );
        }
        return this.#iotaLedgerClient;
    }

    async #initializeSigner() {
        if (!this.#signer) {
            const ledgerClient = await this.#initializeIotaLedgerClient();
            this.#signer = await SignersLedgerSigner.fromDerivationPath(
                this.#derivationPath,
                ledgerClient,
                this.client,
            );
        }
        return this.#signer;
    }

    async #verifyLedgerAddress() {
        // Verify that the connected ledger device matches the expected address
        // This prevents signing with the wrong ledger device
        const actualAddress = await this.getAddress();
        if (actualAddress !== this.#expectedAddress) {
            throw new Error(
                `Ledger address mismatch. Expected: ${this.#expectedAddress}, Got: ${actualAddress}. ` +
                    `Please make sure you have the correct Ledger device connected and unlocked.`,
            );
        }
    }

    async getAddress(): Promise<string> {
        const signer = await this.#initializeSigner();
        return signer.toIotaAddress();
    }

    async getPublicKey(): Promise<Ed25519PublicKey> {
        const signer = await this.#initializeSigner();
        return signer.getPublicKey();
    }

    async signMessage(input: { message: Uint8Array }): Promise<SignedMessage> {
        await this.#verifyLedgerAddress();
        const signer = await this.#initializeSigner();
        const signature = await signer.signPersonalMessage(input.message);
        return signature as SignedMessage;
    }

    async signTransactionBytes(bytes: Uint8Array): Promise<SignedTransaction> {
        await this.#verifyLedgerAddress();
        const signer = await this.#initializeSigner();
        const signature = await signer.signTransaction(bytes);
        return signature as SignedTransaction;
    }

    connect(client: IotaClient) {
        return new LedgerSigner(
            this.#connectToLedger,
            this.#derivationPath,
            this.#expectedAddress,
            client,
        );
    }
}
