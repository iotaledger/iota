// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
const identity = [
    'iota-identity/index',
    {
        type: 'category',
        label: 'Getting Started',
        collapsed: false,
        items: ['iota-identity/getting-started/rust'],
    },
    {
        type: 'category',
        label: 'Explanations',
        items: [
            'iota-identity/explanations/decentralized-identifiers',
            'iota-identity/explanations/verifiable-credentials',
            'iota-identity/explanations/verifiable-presentations',
            'iota-identity/explanations/about-identity-objects',
            'iota-identity/explanations/authenticated-assets',
        ],
    },
    {
        type: 'category',
        label: 'How To',
        items: [
            {
                type: 'category',
                label: 'Decentralized Identifiers (DID)',
                items: [
                    'iota-identity/how-tos/decentralized-identifiers/create',
                    'iota-identity/how-tos/decentralized-identifiers/update',
                    'iota-identity/how-tos/decentralized-identifiers/resolve',
                    'iota-identity/how-tos/decentralized-identifiers/delete',
                ],
            },
            {
                type: 'category',
                label: 'Verifiable Credentials',
                items: [
                    'iota-identity/how-tos/verifiable-credentials/create',
                    'iota-identity/how-tos/verifiable-credentials/revocation',
                    'iota-identity/how-tos/verifiable-credentials/selective-disclosure',
                    'iota-identity/how-tos/verifiable-credentials/zero-knowledge-selective-disclosure',
                ],
            },
            {
                type: 'category',
                label: 'Verifiable Presentations',
                items: ['iota-identity/how-tos/verifiable-presentations/create-and-validate'],
            },
            {
                type: 'category',
                label: 'Domain Linkage',
                items: ['iota-identity/how-tos/domain-linkage/create-and-verify'],
            },
            'iota-identity/how-tos/key-storage',
        ],
    },
    {
        type: 'category',
        label: 'References',
        collapsed: true,
        items: [
            {
                type: 'category',
                label: 'API',
                items: [
                    {
                        type: 'link',
                        label: 'Rust',
                        href: 'https://docs.rs/identity_iota/latest/identity_iota/index.html',
                    },
                ],
            },
            {
                type: 'category',
                label: 'Specifications',
                items: [
                    'references/iota-identity/overview',
                    'references/iota-identity/iota-did-method-spec',
                    'references/iota-identity/revocation-bitmap-2022',
                    'references/iota-identity/revocation-timeframe-2024',
                ],
            },
        ],
    },
    'iota-identity/contribute',
];

module.exports = identity;
