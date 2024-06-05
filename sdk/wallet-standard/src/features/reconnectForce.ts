// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { StandardConnectOutput } from '@wallet-standard/core';

export type SuiReconnectForceVersion = '1.0.0';

export const SuiReconnectForce = 'sui:reconnectForce';

export type StandardReconnectForceFeature = {
    /** Name of the feature. */
    readonly [SuiReconnectForce]: {
        /** Version of the feature implemented by the Wallet. */
        readonly version: SuiReconnectForceVersion;
        /** Method to call to use the feature. */
        readonly reconnect: (input: { origin: string }) => Promise<StandardConnectOutput>;
    };
};
