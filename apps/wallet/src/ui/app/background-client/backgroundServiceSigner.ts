// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type SerializedUIAccount } from '_src/background/accounts/account';
import { bcs } from '@iota/iota-sdk/bcs';
import { type IotaClient } from '@iota/iota-sdk/client';
import { messageWithIntent } from '@iota/iota-sdk/cryptography';
import { toBase64 } from '@iota/iota-sdk/utils';

import type { BackgroundClient } from '.';
import { type SignedMessage, type SignedTransaction, WalletSigner } from '../walletSigner';

export class BackgroundServiceSigner extends WalletSigner {
    readonly #account: SerializedUIAccount;
    readonly #backgroundClient: BackgroundClient;

    constructor(
        account: SerializedUIAccount,
        backgroundClient: BackgroundClient,
        client: IotaClient,
    ) {
        super(client);
        this.#account = account;
        this.#backgroundClient = backgroundClient;
    }

    async getAddress(): Promise<string> {
        return this.#account.address;
    }

    async signMessage(input: { message: Uint8Array }): Promise<SignedMessage> {
        const signature = await this.#backgroundClient.signData(
            this.#account.id,
            messageWithIntent(
                'PersonalMessage',
                bcs.vector(bcs.u8()).serialize(input.message).toBytes(),
            ),
        );

        return {
            bytes: toBase64(input.message),
            signature,
        };
    }

    async signTransactionBytes(bytes: Uint8Array): Promise<SignedTransaction> {
        const signature = await this.#backgroundClient.signData(
            this.#account.id,
            messageWithIntent('TransactionData', bytes),
        );

        return {
            bytes: toBase64(bytes),
            signature,
        };
    }

    connect(client: IotaClient) {
        return new BackgroundServiceSigner(this.#account, this.#backgroundClient, client);
    }
}
