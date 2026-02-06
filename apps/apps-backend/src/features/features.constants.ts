// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Network } from '@iota/iota-sdk/client';
import { normalizeIotaAddress } from '@iota/iota-sdk/utils';
import { RECOGNIZED_COIN_PACKAGES } from './coins.constants';

type FeatureEnabledByNetwork = Record<Network, boolean>;

export const ADDRESSES_ALIASES = {
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
    '0xbeb1ba753fd0bbc0f5470b3948345da6dc870c0421d809cfc3abe95b70f625a7': 'Legacy Migrator',
};

export const KNOWN_ADDRESSES_ALIASES = Object.fromEntries(
    Object.entries(ADDRESSES_ALIASES).map(([address, alias]) => [
        normalizeIotaAddress(address),
        alias,
    ]),
);

export const NAME_ADDRESS_RESOLUTION_FEATURE: FeatureEnabledByNetwork = {
    [Network.Mainnet]: true,
    [Network.Testnet]: true,
    [Network.Devnet]: true,
    [Network.Localnet]: true,
    [Network.Custom]: true,
};

export const RECOGNIZED_PACKAGES = [
    '0x2',
    '0x3',
    '0x1',
    '0x107a',
    '0x0000000000000000000000000000000000000000000000000000000000000002',
    '0x0000000000000000000000000000000000000000000000000000000000000003',
    '0x0000000000000000000000000000000000000000000000000000000000000001',
    '0x000000000000000000000000000000000000000000000000000000000000107a',
    ...RECOGNIZED_COIN_PACKAGES.map((coin) => coin.type.split('::')[0]),
];
