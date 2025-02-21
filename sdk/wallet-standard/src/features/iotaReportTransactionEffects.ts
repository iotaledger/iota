// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { IdentifierString, WalletAccount } from '@wallet-standard/core';

/**
 * A Wallet Standard feature for reporting the effects of a transaction block executed by a dapp
 * The feature allows wallets to updated their caches using the effects of the transaction
 * executed outside of the wallet
 */
export type IotaReportTransactionEffectsFeature = {
    /** Namespace for the feature. */
    'iota:reportTransactionEffects': {
        /** Version of the feature API. */
        version: '1.0.0';
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
