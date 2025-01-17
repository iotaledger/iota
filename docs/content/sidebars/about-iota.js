// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

const aboutIota = [
    'about-iota/about-iota',
    'about-iota/why-move',
    {
        type: 'category',
        label: 'IOTA Architecture',
        link: {
            type: 'doc',
            id: 'about-iota/iota-architecture/iota-architecture',
        },
        items: [
            'about-iota/iota-architecture/iota-security',
            'about-iota/iota-architecture/transaction-lifecycle',
            'about-iota/iota-architecture/consensus',
            'about-iota/iota-architecture/epochs',
            'about-iota/iota-architecture/protocol-upgrades',
            'about-iota/iota-architecture/staking-rewards',
        ],
    },
    {
        type: 'category',
        label: 'Tokenomics',
        link: {
            type: 'doc',
            id: 'about-iota/tokenomics/tokenomics',
        },
        items: [
            'about-iota/tokenomics/iota-token',
            'about-iota/tokenomics/proof-of-stake',
            'about-iota/tokenomics/validators-staking',
            'about-iota/tokenomics/staking-unstaking',
            'about-iota/tokenomics/gas-in-iota',
            'about-iota/tokenomics/gas-pricing',
        ],
    },
    {
        type: 'category',
        label: 'Programs & Funding',
        link: {
            type: 'generated-index',
            title: 'Programs & Funding',
            description: 'Learn about the Programs and Funding available for the IOTA ecosystem.',
            slug: '/about-iota/programs-funding',
        },
        items: [
            {
                type: 'link',
                label: 'IOTA Builders Program',
                href: 'https://iotalabs.io',
                description: 'iotalabs propels the IOTA ecosystem through grants, growth initiatives, builder support, and strategic partnerships. Join us in shaping the future of IOTA—one breakthrough at a time.',
            },
            {
                type: 'link',
                label: 'IOTA Grants',
                href: 'https://iotalabs.io/grants',
                description: 'IOTA Grants by the IOTA Builders Program',
            },
            {
                type: 'link',
                label: 'Tangle Community Treasury',
                href: 'https://www.tangletreasury.org',
                description: 'A Decentralized Community governed Fund to support projects in the IOTA Ecosystem and Support the community',
            },
        ],
    },
    {
        type: 'category',
        label: 'IOTA Wallet',
        items: [
            'about-iota/iota-wallet/getting-started',
            {
                type:'category',
                label:'How To',
                items:[
                    'about-iota/iota-wallet/how-to/basics',
                    'about-iota/iota-wallet/how-to/stake',
                    'about-iota/iota-wallet/how-to/multi-account',
                    'about-iota/iota-wallet/how-to/get-test-tokens',
                    'about-iota/iota-wallet/how-to/integrate-ledger',
                ]
            },
            'about-iota/iota-wallet/FAQ',
        ],
    },
    {
        type: 'category',
        label: 'IOTA Explorer and Bridges',
        items: [
            {
                type: 'link',
                label: 'IOTA Explorer',
                href: 'https://explorer.iota.org/',
            },
            {
                type: 'link',
                label: 'IOTA EVM Explorer',
                href: 'https://explorer.evm.iota.org/',
            },
            {
                type: 'link',
                label: 'IOTA EVM Toolkit',
                href: 'https://explorer.evm.iota.org/',
            },
            {
                type: 'link',
                label: 'IOTA Bridge (Stargate)',
                href: 'https://stargate.finance/bridge?dstChain=iota',
            },
            {
                type: 'link',
                label: 'Shimmer Explorer',
                href: 'https://explorer.shimmer.network/',
            },
            {
                type: 'link',
                label: 'Shimmer EVM Explorer',
                href: 'https://explorer.evm.shimmer.network/',
            },
            {
                type: 'link',
                label: 'Shimmer EVM Toolkit',
                href: 'https://evm-toolkit.evm.shimmer.network/',
            },
            {
                type: 'doc',
                label: 'Shimmer Bridge',
                id: 'about-iota/iota-tools/shimmer-bridge',
            },
        ],
    },
    'about-iota/FAQ',
];
module.exports = aboutIota;
