// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { IdentifierString, WalletAccount } from '@wallet-standard/core';

/** The latest API version of the signTransaction API. */
export type IotaSignTransactionVersion = '2.0.0';

/**
 * A Wallet Standard feature for signing a transaction, and returning the
 * serialized transaction and transaction signature.
 */
export type IotaSignTransactionFeature = {
    /** Namespace for the feature. */
    'iota:signTransaction': {
        /** Version of the feature API. */
        version: IotaSignTransactionVersion;
        signTransaction: IotaSignTransactionMethod;
    };
};

export type IotaSignTransactionMethod = (
    input: IotaSignTransactionInput,
) => Promise<SignedTransaction>;

/** Input for signing transactions. */
export interface IotaSignTransactionInput {
    transaction: { toJSON: () => Promise<string> };
    account: WalletAccount;
    chain: IdentifierString;
    signal?: AbortSignal;
}

/** Output of signing transactions. */

export interface SignedTransaction {
    /** Transaction as base64 encoded bcs. */
    bytes: string;
    /** Base64 encoded signature */
    signature: string;
}

export interface IotaSignTransactionOutput extends SignedTransaction {}
