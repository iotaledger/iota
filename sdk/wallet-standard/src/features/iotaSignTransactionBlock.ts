// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { TransactionBlock } from '@iota/iota-sdk/transactions';
import type { IdentifierString, WalletAccount } from '@wallet-standard/core';

/** The latest API version of the signTransactionBlock API. */
export type IotaSignTransactionBlockVersion = '1.0.0';

/**
 * A Wallet Standard feature for signing a transaction, and returning the
 * serialized transaction and transaction signature.
 */
export type IotaSignTransactionBlockFeature = {
    /** Namespace for the feature. */
    'iota:signTransactionBlock': {
        /** Version of the feature API. */
        version: IotaSignTransactionBlockVersion;
        signTransactionBlock: IotaSignTransactionBlockMethod;
    };
};

export type IotaSignTransactionBlockMethod = (
    input: IotaSignTransactionBlockInput,
) => Promise<IotaSignTransactionBlockOutput>;

/** Input for signing transactions. */
export interface IotaSignTransactionBlockInput {
    transactionBlock: TransactionBlock;
    account: WalletAccount;
    chain: IdentifierString;
}

/** Output of signing transactions. */
export interface IotaSignTransactionBlockOutput extends SignedTransactionBlock {}

export interface SignedTransactionBlock {
    transactionBlockBytes: string;
    signature: string;
}
