// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

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
    if (!wallet.features['iota:signAndExecuteTransaction']) {
        throw new Error(
            `Provided wallet (${wallet.name}) does not support the signAndExecuteTransaction feature.`,
        );
    }

    return wallet.features['iota:signAndExecuteTransaction'].signAndExecuteTransaction(input);
}

export async function signTransaction(
    wallet: WalletWithFeatures<Partial<IotaWalletFeatures>>,
    input: IotaSignTransactionInput,
) {
    if (!wallet.features['iota:signTransaction']) {
        throw new Error(
            `Provided wallet (${wallet.name}) does not support the signTransaction feature.`,
        );
    }

    return wallet.features['iota:signTransaction'].signTransaction(input);
}
