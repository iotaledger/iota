// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type {
    ExecuteTransactionRequestType,
    IotaTransactionBlockResponse,
    IotaTransactionBlockResponseOptions,
} from '@iota/iota-sdk/client';

import type { IotaSignTransactionBlockInput } from './iotaSignTransactionBlock.js';

/** The latest API version of the signAndExecuteTransactionBlock API. */
export type IotaSignAndExecuteTransactionBlockVersion = '1.0.0';

/**
 * A Wallet Standard feature for signing a transaction, and submitting it to the
 * network. The wallet is expected to submit the transaction to the network via RPC,
 * and return the transaction response.
 */
export type IotaSignAndExecuteTransactionBlockFeature = {
    /** Namespace for the feature. */
    'iota:signAndExecuteTransactionBlock': {
        /** Version of the feature API. */
        version: IotaSignAndExecuteTransactionBlockVersion;
        signAndExecuteTransactionBlock: IotaSignAndExecuteTransactionBlockMethod;
    };
};

export type IotaSignAndExecuteTransactionBlockMethod = (
    input: IotaSignAndExecuteTransactionBlockInput,
) => Promise<IotaSignAndExecuteTransactionBlockOutput>;

/** Input for signing and sending transactions. */
export interface IotaSignAndExecuteTransactionBlockInput extends IotaSignTransactionBlockInput {
    /**
     * `WaitForEffectsCert` or `WaitForLocalExecution`, see details in `ExecuteTransactionRequestType`.
     * Defaults to `WaitForLocalExecution` if options.showEffects or options.showEvents is true
     */
    requestType?: ExecuteTransactionRequestType;
    /** specify which fields to return (e.g., transaction, effects, events, etc). By default, only the transaction digest will be returned. */
    options?: IotaTransactionBlockResponseOptions;
}

/** Output of signing and sending transactions. */
export interface IotaSignAndExecuteTransactionBlockOutput extends IotaTransactionBlockResponse {}
