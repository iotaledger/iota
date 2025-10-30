// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    type ChainType,
    getAllNetworks,
    IotaClient,
    type DryRunTransactionBlockResponse,
    type IotaTransactionBlockResponse,
    type IotaTransactionBlockResponseOptions,
} from '@iota/iota-sdk/client';
import { isTransaction, type Transaction } from '@iota/iota-sdk/transactions';
import { fromBase64 } from '@iota/iota-sdk/utils';

export interface SignedTransaction {
    bytes: string;
    signature: string;
}

export type SignedMessage = {
    bytes: string;
    signature: string;
};

export abstract class WalletSigner {
    client: IotaClient;

    constructor(client: IotaClient) {
        this.client = client;
    }

    protected getClient(chain?: ChainType): IotaClient {
        const requestedNetwork = Object.values(getAllNetworks() ?? {}).find(
            (network) => network.chain === chain,
        );
        const client = requestedNetwork
            ? new IotaClient({ url: requestedNetwork.url })
            : this.client;
        return client;
    }

    abstract signMessage(input: { message: Uint8Array }): Promise<SignedMessage>;

    abstract getAddress(): Promise<string>;

    protected async prepareTransaction(
        transaction: Uint8Array | Transaction | string,
        chain?: ChainType,
    ) {
        if (isTransaction(transaction)) {
            // If the sender has not yet been set on the transaction, then set it.
            // NOTE: This allows for signing transactions with mismatched senders, which is important for sponsored transactions.
            if (!transaction.getData().sender) {
                transaction.setSender(await this.getAddress());
            }
            const client = this.getClient(chain);

            return await transaction.build({
                client,
            });
        }

        if (typeof transaction === 'string') {
            return fromBase64(transaction);
        }

        if (transaction instanceof Uint8Array) {
            return transaction;
        }
        throw new Error('Unknown transaction format');
    }

    abstract signTransactionBytes(bytes: Uint8Array): Promise<SignedTransaction>;

    async signTransaction(input: {
        transaction: Uint8Array | Transaction;
        chain?: ChainType;
    }): Promise<SignedTransaction> {
        // Prepare the transaction (sets sender if not already set, builds Transaction objects)
        const bytes = await this.prepareTransaction(input.transaction, input.chain);
        return this.signTransactionBytes(bytes);
    }

    async signAndExecuteTransaction(input: {
        transactionBlock: Uint8Array | Transaction;
        options?: IotaTransactionBlockResponseOptions;
        chain?: ChainType;
    }): Promise<IotaTransactionBlockResponse> {
        const signed = await this.signTransaction({
            transaction: input.transactionBlock,
            chain: input.chain,
        });

        const client = this.getClient(input.chain);

        return client.executeTransactionBlock({
            transactionBlock: signed.bytes,
            signature: signed.signature,
            options: input.options,
        });
    }

    async dryRunTransactionBlock(input: {
        transactionBlock: Transaction | string | Uint8Array;
        chain?: ChainType;
    }): Promise<DryRunTransactionBlockResponse> {
        const client = this.getClient(input.chain);
        return client.dryRunTransactionBlock({
            transactionBlock: await this.prepareTransaction(input.transactionBlock, input.chain),
        });
    }
}
