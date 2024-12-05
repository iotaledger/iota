// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
const identity = [
    
    {
        type: 'category',
        label: 'Identity',
        link: {
            type: 'doc',
            id: 'developer/iota-identity/welcome',
        },
        items: [
            {
                type: 'category',
                label: 'Getting Started',
                collapsed: false,
                items: ['developer/iota-identity/getting-started/rust'],
            },
            {
                type: 'category',
                label: 'Explanations',
                items: [
                    'developer/iota-identity/explanations/decentralized-identifiers',
                    'developer/iota-identity/explanations/verifiable-credentials',
                    'developer/iota-identity/explanations/verifiable-presentations',
                    'developer/iota-identity/explanations/about-identity-objects',
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
            'developer/iota-identity/contribute',
        ],
    },
];

module.exports = identity;
