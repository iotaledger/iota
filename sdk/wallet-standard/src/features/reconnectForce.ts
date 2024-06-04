// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { StandardConnectOutput } from '@wallet-standard/core';

export type StandardReconnectForceVersion = '1.0.0';

export const StandardReconnectForce = 'standard:reconnectForce';

export type StandardReconnectForceFeature = {
    /** Name of the feature. */
    readonly [StandardReconnectForce]: {
        /** Version of the feature implemented by the Wallet. */
        readonly version: StandardReconnectForceVersion;
        /** Method to call to use the feature. */
        readonly reconnect: (input: { origin: string }) => Promise<StandardConnectOutput>;
    };
};
