// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type IotaClient } from '@iota/iota-sdk/client';
import { v4 as uuidv4 } from 'uuid';
import { type SignedMessage, type SignedTransaction, WalletSigner } from './walletSigner';
import type { KeystoneAccountSerializedUI } from '_src/background/accounts/keystoneAccount';
import { KeystoneIotaSDK, type UR } from '@keystonehq/keystone-sdk';
import { bcs, toBase64, toHex } from '@iota/bcs';
import { messageWithIntent } from '@iota/iota-sdk/cryptography';
import { SignatureType } from './components/keystone/KeystoneProvider';

export class KeystoneSigner extends WalletSigner {
    readonly #derivationPath: string;
    readonly #address: string;
    readonly #requestSignature: (ur: UR, signatureType?: SignatureType) => Promise<string>;
    readonly #masterFingerprint: string;

    constructor(
        requestSignature: (ur: UR, signatureType?: SignatureType) => Promise<string>,
        { address, masterFingerprint, derivationPath }: KeystoneAccountSerializedUI,
        client: IotaClient,
    ) {
        super(client);
        this.#derivationPath = derivationPath;
        this.#address = address;
        this.#requestSignature = requestSignature;
        this.#masterFingerprint = masterFingerprint;
    }

    async getAddress(): Promise<string> {
        return this.#address;
    }

    async signMessage(input: { message: Uint8Array }): Promise<SignedMessage> {
        const iotaSignRequest = {
            requestId: uuidv4(),
            intentMessage: toHex(
                messageWithIntent(
                    'PersonalMessage',
                    bcs.vector(bcs.u8()).serialize(input.message).toBytes(),
                ),
            ),
            accounts: [
                {
                    path: this.#derivationPath,
                    xfp: this.#masterFingerprint,
                    address: this.#address,
                },
            ],
            origin: 'IOTA Wallet',
        };

        const ur = new KeystoneIotaSDK().generateSignRequest(iotaSignRequest);
        const signature = await this.#requestSignature(ur, SignatureType.Message);
        return {
            bytes: toBase64(input.message),
            signature,
        };
    }

    async signTransactionBytes(bytes: Uint8Array): Promise<SignedTransaction> {
        const iotaSignRequest = {
            requestId: uuidv4(),
            intentMessage: toHex(messageWithIntent('TransactionData', bytes)),
            accounts: [
                {
                    path: this.#derivationPath,
                    xfp: this.#masterFingerprint,
                    address: this.#address,
                },
            ],
            origin: 'IOTA Wallet',
        };

        const ur = new KeystoneIotaSDK().generateSignRequest(iotaSignRequest);
        const signature = await this.#requestSignature(ur, SignatureType.Transaction);
        return {
            bytes: toBase64(bytes),
            signature,
        };
    }
}
