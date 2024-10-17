// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { bcs } from '@iota/iota-sdk/bcs';
import { Transaction } from '@iota/iota-sdk/transactions';
import { fromB64, toB64 } from '@iota/iota-sdk/utils';
import type { WalletWithFeatures } from '@wallet-standard/core';

import type {
    IotaSignAndExecuteTransactionInput,
    IotaSignTransactionInput,
    IotaWalletFeatures,
} from './features/index.js';

declare module '@wallet-standard/core' {
    export interface Wallet {
        /**
         * Unique identifier of the Wallet.
         *
         * If not provided, the wallet name will be used as the identifier.
         */
        readonly id?: string;
    }

    export interface StandardConnectOutput {
        supportedIntents?: string[];
    }
}

export type { Wallet } from '@wallet-standard/core';

export async function signAndExecuteTransaction(
    wallet: WalletWithFeatures<Partial<IotaWalletFeatures>>,
    input: IotaSignAndExecuteTransactionInput,
) {
    if (wallet.features['iota:signAndExecuteTransaction']) {
        return wallet.features['iota:signAndExecuteTransaction'].signAndExecuteTransaction(input);
    }

    if (!wallet.features['iota:signAndExecuteTransactionBlock']) {
        throw new Error(
            `Provided wallet (${wallet.name}) does not support the signAndExecuteTransaction feature.`,
        );
    }

    const { signAndExecuteTransactionBlock } =
        wallet.features['iota:signAndExecuteTransactionBlock'];

    const transactionBlock = Transaction.from(await input.transaction.toJSON());
    const { digest, rawEffects, rawTransaction } = await signAndExecuteTransactionBlock({
        account: input.account,
        chain: input.chain,
        transactionBlock,
        options: {
            showRawEffects: true,
            showRawInput: true,
        },
    });

    const [
        {
            txSignatures: [signature],
            intentMessage: { value: bcsTransaction },
        },
    ] = bcs.SenderSignedData.parse(fromB64(rawTransaction!));

    const bytes = bcs.TransactionData.serialize(bcsTransaction).toBase64();

    return {
        digest,
        signature,
        bytes,
        effects: toB64(new Uint8Array(rawEffects!)),
    };
}

export async function signTransaction(
    wallet: WalletWithFeatures<Partial<IotaWalletFeatures>>,
    input: IotaSignTransactionInput,
) {
    if (wallet.features['iota:signTransaction']) {
        return wallet.features['iota:signTransaction'].signTransaction(input);
    }

    if (!wallet.features['iota:signTransactionBlock']) {
        throw new Error(
            `Provided wallet (${wallet.name}) does not support the signTransaction feature.`,
        );
    }

    const { signTransactionBlock } = wallet.features['iota:signTransactionBlock'];

    const transaction = Transaction.from(await input.transaction.toJSON());
    const { transactionBlockBytes, signature } = await signTransactionBlock({
        transactionBlock: transaction,
        account: input.account,
        chain: input.chain,
    });

    return { bytes: transactionBlockBytes, signature };
}
