// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

const iotaSDK = [
    {
        type: 'category',
        label: 'Getting Started',
        items: [
            'developer/iota-sdk/getting-started/rust',
            'developer/iota-sdk/getting-started/go',
            'developer/iota-sdk/getting-started/kotlin',
            'developer/iota-sdk/getting-started/python',
        ],
    },
    {
        type: 'category',
        label: 'Explantations',
        items: [
            'developer/iota-sdk/explanations/place-holder',
        ],
    },
    {
        type: 'category',
        label: 'How To',
        items: [
            {
                type: 'category',
                label: 'Accounts and Addresses',
                items: [
                    'developer/iota-sdk/how-tos/accounts-and-addresses/create-mnemonic',
                    'developer/rust-sdk/how-tos/accounts-and-addresses/address-from-mnemonic',
                    'developer/rust-sdk/how-tos/accounts-and-addresses/coin-balance',
                ],
            },
            {
                type: 'category',
                label: 'Transactions',
                items: [
                    'developer/rust-sdk/how-tos/transactions/prepare-send-iota',
                    'developer/rust-sdk/how-tos/transactions/sign-send-iota',
                ],
            },
        ],
    },
    {
        type: 'category',
        label: 'API Reference',
        items: [
            'developer/iota-sdk/references/place-holder',
        ],
    },
];

module.exports = iotaSDK;