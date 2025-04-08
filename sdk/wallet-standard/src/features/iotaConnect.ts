// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { StandardConnectInput, StandardConnectOutput } from '@wallet-standard/core';

export const IotaConnect = 'iota:connect' as const;

export type IotaConnectFeatureVersion = '1.0.0';

export type IotaConnectFeature = {
    // Might not be supported by all the wallets.
    readonly [IotaConnect]: {
        readonly version: IotaConnectFeatureVersion;
        readonly connect: IotaConnectMethod;
    };
};

export type IotaConnectInput = {
    forceReinitialize?: boolean;
};

export type IotaConnectMethodInputs = StandardConnectInput & IotaConnectInput;

export type IotaConnectMethod = (input?: IotaConnectMethodInputs) => Promise<StandardConnectOutput>;
