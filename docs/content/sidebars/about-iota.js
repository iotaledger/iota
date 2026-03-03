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
            'about-iota/iota-architecture/sequencing',
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
                label: 'Business Innovation Program',
                href: 'https://blog.iota.org/iota-business-innovation-program',
                description:
                    'Technical and financial support for real-world impact',
            },
            {
                type: 'link',
                label: 'IOTA Grants',
                href: 'https://iotalabs.io/grants',
                description: 'Apply for a grant from iotalabs and turn your wildest dApp dreams into reality',
            },
            {
                type: 'doc',
                id: 'about-iota/programs-funding/bounty-program',
                label: 'IOTA Bounty Program',
            }
        ],
    },
    'about-iota/FAQ',
];
module.exports = aboutIota;
