// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
const identity = [
    'developer/iota-identity/index',
    {
        type: 'category',
        label: 'Getting Started',
        collapsed: false,
        items: [
            'developer/iota-identity/getting-started/rust',
            'developer/iota-identity/getting-started/wasm',
            'developer/iota-identity/getting-started/local-network-setup',
            'developer/iota-identity/getting-started/universal-resolver'
        ],
    },
    {
        type: 'category',
        label: 'Explanations',
        items: [
            'developer/iota-identity/explanations/decentralized-identifiers',
            'developer/iota-identity/explanations/verifiable-credentials',
            'developer/iota-identity/explanations/verifiable-presentations',
            'developer/iota-identity/explanations/about-identity-objects',
            'developer/iota-identity/explanations/authenticated-assets',
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
                    'developer/iota-identity/how-tos/decentralized-identifiers/create',
                    'developer/iota-identity/how-tos/decentralized-identifiers/update',
                    'developer/iota-identity/how-tos/decentralized-identifiers/resolve',
                    'developer/iota-identity/how-tos/decentralized-identifiers/delete',
                ],
            },
            {
                type: 'category',
                label: 'Verifiable Credentials',
                items: [
                    'developer/iota-identity/how-tos/verifiable-credentials/create',
                    'developer/iota-identity/how-tos/verifiable-credentials/revocation',
                    'developer/iota-identity/how-tos/verifiable-credentials/selective-disclosure',
                    'developer/iota-identity/how-tos/verifiable-credentials/zero-knowledge-selective-disclosure',
                ],
            },
            {
                type: 'category',
                label: 'Verifiable Presentations',
                items: ['developer/iota-identity/how-tos/verifiable-presentations/create-and-validate'],
            },
            {
                type: 'category',
                label: 'Domain Linkage',
                items: ['developer/iota-identity/how-tos/domain-linkage/create-and-verify'],
            },
            'developer/iota-identity/how-tos/key-storage',
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
                        href: 'https://iotaledger.github.io/identity/identity_iota/index.html',
                    },
                    {
                        type: 'link',
                        label: 'Wasm',
                        href: '/iota-identity/references/wasm/api_ref',
                    },
                ],
            },
            {
                type: 'category',
                label: 'Specifications',
                items: [
                    'developer/iota-identity/references/overview',
                    'developer/iota-identity/references/iota-did-method-spec',
                    'developer/iota-identity/references/revocation-bitmap-2022',
                    'developer/iota-identity/references/revocation-timeframe-2024',
                ],
            },
        ],
    },
    'developer/iota-identity/contribute',
];

module.exports = identity;
