// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

import type { WalletAccount } from '@wallet-standard/core';

/** The latest API version of the advancedConnect API. */
export type SuiAdvancedConnectVersion = '1.0.0';

/**
 * A Wallet Standard feature for signing a transaction, and returning the
 * serialized transaction and transaction signature.
 */
export type SuiAdvancedConnectFeature = {
    /** Namespace for the feature. */
    'sui:advancedConnect': {
        /** Version of the feature API. */
        version: SuiAdvancedConnectVersion;
        advancedConnect: SuiAdvancedConnectMethod;
    };
};

export type SuiAdvancedConnectMethod = (
    input: SuiAdvancedConnectInput,
) => Promise<SuiAdvancedConnectOutput>;

/** Input for signing transactions. */
export interface SuiAdvancedConnectInput {
    force: boolean;
}

/** Output of signing transactions. */
export interface SuiAdvancedConnectOutput {
    /** List of accounts in the {@link "@wallet-standard/base".Wallet} that the app has been authorized to use. */
    readonly accounts: readonly WalletAccount[];
}

