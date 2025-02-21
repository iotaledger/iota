// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { IotaTransactionBlockResponseOptions } from '@iota/iota-sdk/client';
import type { SignedTransaction, IotaSignTransactionInput } from './iotaSignTransaction.js';

/** The latest API version of the signAndExecuteTransactionBlock API. */
export type IotaSignAndExecuteTransactionVersion = '2.0.0';

/**
 * A Wallet Standard feature for signing a transaction, and submitting it to the
 * network. The wallet is expected to submit the transaction to the network via RPC,
 * and return the transaction response.
 */
export type IotaSignAndExecuteTransactionFeature = {
    /** Namespace for the feature. */
    'iota:signAndExecuteTransaction': {
        /** Version of the feature API. */
        version: IotaSignAndExecuteTransactionVersion;
        signAndExecuteTransaction: IotaSignAndExecuteTransactionMethod;
    };
};

export type IotaSignAndExecuteTransactionMethod = (
    input: IotaSignAndExecuteTransactionInput,
) => Promise<IotaSignAndExecuteTransactionOutput>;

/** Input for signing and sending transactions. */
export interface IotaSignAndExecuteTransactionInput extends IotaSignTransactionInput {
    /** specify which fields to return (e.g., transaction, effects, events, etc). By default, only the transaction digest will be returned. */
    options?: IotaTransactionBlockResponseOptions;
}

/** Output of signing and sending transactions. */
export interface IotaSignAndExecuteTransactionOutput extends SignedTransaction {
    digest: string;
    /** Transaction effects as base64 encoded bcs. */
    effects: string;
}
