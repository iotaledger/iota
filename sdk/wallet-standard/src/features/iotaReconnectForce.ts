// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { WalletAccount } from '@wallet-standard/core';

export type IotaReconnectForceVersion = '1.0.0';

export type IotaReconnectForceOutput = {
    readonly accounts: readonly WalletAccount[];
};

export type IotaReconnectForceMethod = (input: {
    origin: string;
}) => Promise<IotaReconnectForceOutput>;

export const IotaReconnectForce = 'iota:reconnectForce';

export type IotaReconnectForceFeature = {
    /** Name of the feature. */
    readonly [IotaReconnectForce]: {
        /** Version of the feature implemented by the Wallet. */
        readonly version: IotaReconnectForceVersion;
        /** Method to call to use the feature. */
        readonly reconnect: IotaReconnectForceMethod;
    };
};
