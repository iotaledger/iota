// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type IotaLedgerClient from '@iota/ledgerjs-hw-app-iota';
import { type IotaClient } from '@iota/iota-sdk/client';
import { type Ed25519PublicKey } from '@iota/iota-sdk/keypairs/ed25519';
import { LedgerSigner as SignersLedgerSigner } from '@iota/signers/ledger';
import { type Transaction } from '@iota/iota-sdk/transactions';

import { type SignedMessage, type SignedTransaction, WalletSigner } from './walletSigner';

export class LedgerSigner extends WalletSigner {
    #iotaLedgerClient: IotaLedgerClient | null;
    #signer: SignersLedgerSigner | null = null;
    readonly #connectToLedger: () => Promise<IotaLedgerClient>;
    readonly #derivationPath: string;

    constructor(
        connectToLedger: () => Promise<IotaLedgerClient>,
        derivationPath: string,
        client: IotaClient,
    ) {
        super(client);
        this.#connectToLedger = connectToLedger;
        this.#iotaLedgerClient = null;
        this.#derivationPath = derivationPath;
    }

    async #initializeIotaLedgerClient() {
        if (!this.#iotaLedgerClient) {
            // We want to make sure that there's only one connection established per Ledger signer
            // instance since some methods make multiple calls like getAddress and signData
            this.#iotaLedgerClient = await this.#connectToLedger();
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

    async getAddress(): Promise<string> {
        const signer = await this.#initializeSigner();
        return signer.toIotaAddress();
    }

    async getPublicKey(): Promise<Ed25519PublicKey> {
        const signer = await this.#initializeSigner();
        return signer.getPublicKey();
    }

    async signMessage(input: { message: Uint8Array }): Promise<SignedMessage> {
        const signer = await this.#initializeSigner();
        const signature = await signer.signPersonalMessage(input.message);
        return signature as SignedMessage;
    }

    async signTransaction(input: {
        transaction: Uint8Array | Transaction;
    }): Promise<SignedTransaction> {
        const signer = await this.#initializeSigner();
        const bytes = await this.prepareTransaction(input.transaction);
        const signature = await signer.signTransaction(bytes);
        return signature as SignedTransaction;
    }

    connect(client: IotaClient) {
        return new LedgerSigner(this.#connectToLedger, this.#derivationPath, client);
    }
}
