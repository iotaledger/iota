// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { IdentifierString, WalletAccount } from '@wallet-standard/core';

/** Name of the feature. */
export const IotaReportTransactionEffects = 'iota:reportTransactionEffects';

/** The latest API version of the reportTransactionEffects API. */
export type IotaReportTransactionEffectsVersion = '1.0.0';

/**
 * A Wallet Standard feature for reporting the effects of a transaction block executed by a dapp
 * The feature allows wallets to updated their caches using the effects of the transaction
 * executed outside of the wallet
 */
export type IotaReportTransactionEffectsFeature = {
    /** Namespace for the feature. */
    [IotaReportTransactionEffects]: {
        /** Version of the feature API. */
        version: IotaReportTransactionEffectsVersion;
        reportTransactionEffects: IotaReportTransactionEffectsMethod;
    };
};

export type IotaReportTransactionEffectsMethod = (
    input: IotaReportTransactionEffectsInput,
) => Promise<void>;

/** Input for signing transactions. */
export interface IotaReportTransactionEffectsInput {
    account: WalletAccount;
    chain: IdentifierString;
    /** Transaction effects as base64 encoded bcs. */
    effects: string;
}
