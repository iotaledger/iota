// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { StandardConnectOutput } from '@wallet-standard/core';

export type IotaReconnectForceVersion = '1.0.0';

export const IotaReconnectForce = 'iota:reconnectForce';

export type StandardReconnectForceFeature = {
    /** Name of the feature. */
    readonly [IotaReconnectForce]: {
        /** Version of the feature implemented by the Wallet. */
        readonly version: IotaReconnectForceVersion;
        /** Method to call to use the feature. */
        readonly reconnect: (input: { origin: string }) => Promise<StandardConnectOutput>;
    };
};
