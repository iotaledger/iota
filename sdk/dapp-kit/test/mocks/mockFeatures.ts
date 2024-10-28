// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { IdentifierRecord, IotaFeatures, IotaSignMessageFeature } from '@iota/wallet-standard';

export const signMessageFeature: IotaSignMessageFeature = {
    'iota:signMessage': {
        version: '1.0.0',
        signMessage: vi.fn(),
    },
};

export const superCoolFeature: IdentifierRecord<unknown> = {
    'my-dapp:super-cool-feature': {
        version: '1.0.0',
        superCoolFeature: vi.fn(),
    },
};

export const iotaFeatures: IotaFeatures = {
    ...signMessageFeature,
    'iota:signPersonalMessage': {
        version: '1.0.0',
        signPersonalMessage: vi.fn(),
    },
    'iota:signTransactionBlock': {
        version: '1.0.0',
        signTransactionBlock: vi.fn(),
    },
    'iota:signTransaction': {
        version: '2.0.0',
        signTransaction: vi.fn(),
    },
    'iota:signAndExecuteTransactionBlock': {
        version: '1.0.0',
        signAndExecuteTransactionBlock: vi.fn(),
    },
    'iota:signAndExecuteTransaction': {
        version: '2.0.0',
        signAndExecuteTransaction: vi.fn(),
    },
    'iota:reportTransactionEffects': {
        version: '1.0.0',
        reportTransactionEffects: vi.fn(),
    },
};
