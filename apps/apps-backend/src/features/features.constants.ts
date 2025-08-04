// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Network } from '@iota/iota-sdk/client';

type FeatureEnabledByNetwork = Record<Network, boolean>;

export const KNOWN_ADDRESSES = {
    '0x0': 'IOTA System Account',
    '0x1': 'Move stdlib Package',
    '0x2': 'IOTA Framework Package',
    '0x3': 'IOTA System Package',
    '0x5': 'IOTA System Object',
    '0x6': 'IOTA System Clock',
    '0x7': 'IOTA Authenticator Object',
    '0x8': 'IOTA Randonmness Object',
    '0x9': 'Bridge Object',
    '0x107a': 'Stardust Package',
    '0xb': 'Bridge Package',
    '0x403': 'IOTA Denylist Object',
    '0x7b4a34f6a011794f0ecbe5e5beb96102d3eef6122eb929b9f50a8d757bfbdd67': 'IOTA EVM',
};

export const NAME_ADDRESS_RESOLUTION_FEATURE: FeatureEnabledByNetwork = {
    [Network.Mainnet]: false,
    [Network.Testnet]: false,
    [Network.Devnet]: true,
    [Network.Localnet]: false,
    [Network.Custom]: false,
};
