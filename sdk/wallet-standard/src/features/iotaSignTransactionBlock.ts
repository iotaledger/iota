// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Transaction } from '@iota/iota-sdk/transactions';
import type { IdentifierString, WalletAccount } from '@wallet-standard/core';

/** The latest API version of the signTransactionBlock API. */
export type IotaSignTransactionBlockVersion = '1.0.0';

/**
 * @deprecated Use `iota:signTransaction` instead.
 *
 * A Wallet Standard feature for signing a transaction, and returning the
 * serialized transaction and transaction signature.
 */
export type IotaSignTransactionBlockFeature = {
    /** Namespace for the feature. */
    'iota:signTransactionBlock': {
        /** Version of the feature API. */
        version: IotaSignTransactionBlockVersion;
        /** @deprecated Use `iota:signTransaction` instead. */
        signTransactionBlock: IotaSignTransactionBlockMethod;
    };
};

/** @deprecated Use `iota:signTransaction` instead. */
export type IotaSignTransactionBlockMethod = (
    input: IotaSignTransactionBlockInput,
) => Promise<IotaSignTransactionBlockOutput>;

/** Input for signing transactions. */
export interface IotaSignTransactionBlockInput {
    transactionBlock: Transaction;
    account: WalletAccount;
    chain: IdentifierString;
}

/** Output of signing transactions. */
export interface IotaSignTransactionBlockOutput extends SignedTransactionBlock {}

export interface SignedTransactionBlock {
    /** Transaction as base64 encoded bcs. */
    transactionBlockBytes: string;
    /** Base64 encoded signature */
    signature: string;
}
