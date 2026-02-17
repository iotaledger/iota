// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export interface RecognizedDapp {
    name: string;
    link: string;
    icon: string;
    tags?: string[];
}

export const RECOGNIZED_DAPPS: RecognizedDapp[] = [
    {
        name: 'Wallet Dashboard',
        link: 'https://wallet-dashboard.iota.org/',
        icon: 'https://iota.org/logo.png',
        tags: ['Official', 'Utility'],
    },
    {
        name: 'IOTA Names',
        link: 'https://iotanames.com/',
        icon: 'https://files.iota.org/media/tooling/dapps/iotanames-logo.png',
        tags: ['Official', 'Utility'],
    },
    {
        name: 'EVM Bridge',
        link: 'https://evm-bridge.iota.org/',
        icon: 'https://iota.org/logo.png',
        tags: ['Official', 'EVM', 'Bridge'],
    },
    {
        name: 'Swirl',
        link: 'https://swirlstake.com/',
        icon: 'https://files.iota.org/media/tooling/dapps/swirlstake-logo.png',
        tags: ['DeFi', 'LST'],
    },
    {
        name: 'TokenLabs',
        link: 'https://tokenlabs.network/',
        icon: 'https://files.iota.org/media/tooling/dapps/tokenlabs-logo.png',
        tags: ['DeFi', 'LST'],
    },
    {
        name: 'Virtue',
        link: 'https://virtue.money/',
        icon: 'https://files.iota.org/media/tooling/dapps/virtue-logo.png',
        tags: ['DeFi', 'CDP'],
    },
    {
        name: 'Pools',
        link: 'https://pools.finance/',
        icon: 'https://files.iota.org/media/tooling/dapps/pools-logo.png',
        tags: ['DeFi', 'DEX'],
    },
    {
        name: 'Cyberperp',
        link: 'https://cyberperp.io/',
        icon: 'https://files.iota.org/media/tooling/dapps/cyberperp-logo.png',
        tags: ['DeFi', 'EVM', 'DEX'],
    },
    {
        name: 'LiquidLink',
        link: 'https://iota.liquidlink.io/',
        icon: 'https://files.iota.org/media/tooling/dapps/liquidlink-logo.png',
        tags: ['DeFi', 'Utility'],
    },
];
