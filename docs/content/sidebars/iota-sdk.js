// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

const iotaSDK = [
    {
        type: 'category',
        label: 'Getting Started',
        link: {
            type: 'generated-index',
            slug: 'developer/iota-sdk/getting-started',
        },
        items: [
            'developer/iota-sdk/getting-started/rust',
            'developer/iota-sdk/getting-started/go',
            'developer/iota-sdk/getting-started/kotlin',
            'developer/iota-sdk/getting-started/python',
        ],
    },
    {
        type: 'category',
        label: 'Explanations',
        link: {
            type: 'generated-index',
            slug: 'developer/iota-sdk/explanations',
        },
        items: [
            'developer/iota-sdk/explanations/place-holder',
        ],
    },
    {
        type: 'category',
        label: 'How To',
        link: {
            type: 'generated-index',
            slug: 'developer/iota-sdk/how-tos',
        },
        items: [
            {
                type: 'category',
                label: 'Accounts and Addresses',
                link: {
                    type: 'generated-index',
                    slug: 'developer/iota-sdk/how-tos/accounts-and-addresses',
                },
                items: [
                    'developer/iota-sdk/how-tos/accounts-and-addresses/create-mnemonic',
                ],
            },
        ],
    },
    {
        type: 'category',
        label: 'API Reference',
        link: {
            type: 'generated-index',
            slug: 'developer/iota-sdk/references',
        },
        items: [
            'developer/iota-sdk/references/place-holder',
        ],
    },
];

module.exports = iotaSDK;