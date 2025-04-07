// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type {
    StandardConnect,
    StandardConnectInput,
    StandardConnectOutput,
} from '@wallet-standard/core';

/** The latest API version of the signTransaction API. */
export type StandardConnectExtendedVersion = '1.0.0';

/**
 * A Wallet Standard feature for signing a transaction, and returning the
 * serialized transaction and transaction signature.
 */
export type StandardConnectExtendedFeature = {
    /** Name of the feature. */
    readonly [StandardConnect]: {
        /** Version of the feature implemented by the Wallet. */
        readonly version: StandardConnectExtendedVersion;
        /** Method to call to use the feature. */
        readonly connect: StandardConnectExtendedMethod;
    };
};

export type StandardConnectExtendedMethod = (
    input?: StandardConnectInput & {
        forceReinitialize?: boolean;
    },
) => Promise<StandardConnectOutput>;
