// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

const aboutIota = [
    'about-iota/about-iota',
    {
        type: 'category',
        label: 'IOTA Architecture',
        collapsed: false,
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
        collapsed: false,
        link: {
            type: 'doc',
            id: 'about-iota/tokenomics/tokenomics',
        },
        items: [
            'about-iota/tokenomics/iota-token',
            'about-iota/tokenomics/smr-token',
            'about-iota/tokenomics/proof-of-stake',
            'about-iota/tokenomics/validators-staking',
            'about-iota/tokenomics/staking-unstaking',
            'about-iota/tokenomics/gas-in-iota',
            'about-iota/tokenomics/gas-pricing',
        ],
    },
    {
        type: 'category',
        label: 'Expert topics',
        items: [
            {
                type: 'category',
                label: 'Execution Architecture',
                link: {
                    type: 'doc',
                    id: 'about-iota/execution-architecture/execution-layer',
                },
                items: [
                    'about-iota/execution-architecture/iota-execution',
                    'about-iota/execution-architecture/adapter',
                    'about-iota/execution-architecture/natives',
                ],
            },
        ],
    },
];
module.exports = aboutIota;
