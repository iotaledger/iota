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
            'about-iota/iota-architecture/validator-committee',
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
        label: 'Wallets',
        link: {
            type: 'generated-index',
            title: 'IOTA Wallets',
            description: 'Learn about the different wallets available for IOTA.',
            slug: '/about-iota/wallets',
        },
        items: [
            {
                type: 'category',
                label: 'IOTA Wallet',
                description: 'The official IOTA Wallet.',
                items: [
                    'about-iota/iota-wallet/getting-started',
                    {
                        type: 'category',
                        label: 'How To',
                        items: [
                            'about-iota/iota-wallet/how-to/basics',
                            'about-iota/iota-wallet/how-to/stake',
                            'about-iota/iota-wallet/how-to/multi-account',
                            'about-iota/iota-wallet/how-to/get-test-tokens',
                            'about-iota/iota-wallet/how-to/integrate-ledger',
                        ],
                    },
                    'about-iota/iota-wallet/FAQ',
                ],
            },
            {
                type: 'link',
                label: 'Nightly Wallet',
                href: 'https://nightly.app/download',
                description: 'Nightly provides a browser extension and mobile app for IOTA.',
            }
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
                description:
                    'iotalabs propels the IOTA ecosystem through grants, growth initiatives, builder support, and strategic partnerships. Join us in shaping the future of IOTA—one breakthrough at a time.',
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
                description:
                    'A Decentralized Community governed Fund to support projects in the IOTA Ecosystem and Support the community',
            },
            {
                type: 'link',
                label: 'Business Innovation Program',
                href: 'https://blog.iota.org/iota-business-innovation-program',
                description:
                    'A funding initiative to accelerate real-world adoption of IOTA',
            },
        ],
    },
    'about-iota/FAQ',
];
module.exports = aboutIota;
