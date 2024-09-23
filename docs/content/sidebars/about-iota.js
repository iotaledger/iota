// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

const aboutIota = [
    'about-iota/about-iota',
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
        label: 'IOTA Wallet',
        items: [
            'about-iota/iota-wallet/getting-started',
            'about-iota/iota-wallet/tutorials',
            'about-iota/iota-wallet/multi-account',
            'about-iota/iota-wallet/staking',
            'about-iota/iota-wallet/develop',
            'about-iota/iota-wallet/integrations',
            'about-iota/iota-wallet/FAQ',
        ],
    },
];
module.exports = aboutIota;
