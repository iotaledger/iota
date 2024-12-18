// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { bcs } from '@iota/iota-sdk/bcs';
import {
    type DryRunTransactionBlockResponse,
    type ExecuteTransactionRequestType,
    type IotaClient,
    type IotaTransactionBlockResponse,
    type IotaTransactionBlockResponseOptions,
} from '@iota/iota-sdk/client';
import { messageWithIntent } from '@iota/iota-sdk/cryptography';
import { isTransaction, type Transaction } from '@iota/iota-sdk/transactions';
import { fromBase64, toBase64 } from '@iota/iota-sdk/utils';

export type SignedTransaction = {
    transactionBlockBytes: string;
    signature: string;
};

export type SignedMessage = {
    messageBytes: string;
    signature: string;
};

export abstract class WalletSigner {
    client: IotaClient;

    constructor(client: IotaClient) {
        this.client = client;
    }

    abstract signData(data: Uint8Array, clientIdentifier?: string): Promise<string>;

    abstract getAddress(): Promise<string>;

    async signMessage(
        input: { message: Uint8Array },
        clientIdentifier?: string,
    ): Promise<SignedMessage> {
        const signature = await this.signData(
            messageWithIntent(
                'PersonalMessage',
                bcs.vector(bcs.u8()).serialize(input.message).toBytes(),
            ),
        );

        return {
            messageBytes: toBase64(input.message),
            signature,
        };
    }

    protected async prepareTransactionBlock(transactionBlock: Uint8Array | Transaction | string) {
        if (isTransaction(transactionBlock)) {
            // If the sender has not yet been set on the transaction, then set it.
            // NOTE: This allows for signing transactions with mis-matched senders, which is important for sponsored transactions.
            transactionBlock.setSenderIfNotSet(await this.getAddress());
            return await transactionBlock.build({
                client: this.client,
            });
        }

        if (typeof transactionBlock === 'string') {
            return fromBase64(transactionBlock);
        }

        if (transactionBlock instanceof Uint8Array) {
            return transactionBlock;
        }
        throw new Error('Unknown transaction format');
    }

    async signTransactionBlock(
        input: {
            transactionBlock: Uint8Array | Transaction;
        },
        clientIdentifier?: string,
    ): Promise<SignedTransaction> {
        const bytes = await this.prepareTransactionBlock(input.transactionBlock);
        const signature = await this.signData(messageWithIntent('TransactionData', bytes));

        return {
            transactionBlockBytes: toBase64(bytes),
            signature,
        };
    }

    async signAndExecuteTransactionBlock(
        input: {
            transactionBlock: Uint8Array | Transaction;
            options?: IotaTransactionBlockResponseOptions;
            requestType?: ExecuteTransactionRequestType;
        },
        clientIdentifier?: string,
    ): Promise<IotaTransactionBlockResponse> {
        const bytes = await this.prepareTransactionBlock(input.transactionBlock);
        const signed = await this.signTransactionBlock({
            transactionBlock: bytes,
        });

        return this.client.executeTransactionBlock({
            transactionBlock: bytes,
            signature: signed.signature,
            options: input.options,
            requestType: input.requestType,
        });
    }

    async dryRunTransactionBlock(input: {
        transactionBlock: Transaction | string | Uint8Array;
    }): Promise<DryRunTransactionBlockResponse> {
        return this.client.dryRunTransactionBlock({
            transactionBlock: await this.prepareTransactionBlock(input.transactionBlock),
        });
    }
}
