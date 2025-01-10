// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type IotaLedgerClient from '@iota/ledgerjs-hw-app-iota';
import { type IotaClient } from '@iota/iota-sdk/client';
import { toSerializedSignature, type SignatureScheme } from '@iota/iota-sdk/cryptography';
import { Ed25519PublicKey } from '@iota/iota-sdk/keypairs/ed25519';

import { WalletSigner } from './walletSigner';

export class LedgerSigner extends WalletSigner {
    #iotaLedgerClient: IotaLedgerClient | null;
    readonly #connectToLedger: () => Promise<IotaLedgerClient>;
    readonly #derivationPath: string;
    readonly #signatureScheme: SignatureScheme = 'ED25519';

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

    async getAddress(): Promise<string> {
        const ledgerClient = await this.#initializeIotaLedgerClient();
        const publicKeyResult = await ledgerClient.getPublicKey(this.#derivationPath);
        const publicKey = new Ed25519PublicKey(publicKeyResult.publicKey);
        return publicKey.toIotaAddress();
    }

    async getPublicKey(): Promise<Ed25519PublicKey> {
        const ledgerClient = await this.#initializeIotaLedgerClient();
        const { publicKey } = await ledgerClient.getPublicKey(this.#derivationPath);
        return new Ed25519PublicKey(publicKey);
    }

    async signData(data: Uint8Array): Promise<string> {
        const ledgerClient = await this.#initializeIotaLedgerClient();
        const { signature } = await ledgerClient.signTransaction(this.#derivationPath, data);
        const publicKey = await this.getPublicKey();
        return toSerializedSignature({
            signature,
            signatureScheme: this.#signatureScheme,
            publicKey,
        });
    }

    connect(client: IotaClient) {
        return new LedgerSigner(this.#connectToLedger, this.#derivationPath, client);
    }
}
