// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

const developer = [
    'developer/developer',
    'developer/network-overview',
    {
        type: 'category',
        label: 'Getting Started',
        collapsed: true,
        link: {
            type: 'doc',
            id: 'developer/getting-started/getting-started',
        },
        items: [
            'developer/getting-started/iota-environment',
            'developer/getting-started/install-iota',
            'developer/getting-started/connect',
            'developer/getting-started/local-network',
            'developer/getting-started/get-address',
            'developer/getting-started/get-coins',
            'developer/getting-started/create-a-package',
            'developer/getting-started/create-a-module',
            'developer/getting-started/build-test',
            'developer/getting-started/publish',
            'developer/getting-started/debug',
            'developer/getting-started/client-tssdk',
            'developer/getting-started/coffee-example',
        ],
    },
    {
        type: 'category',
        label: 'SDKs & CLI',
        items: [
            {
                type: 'category',
                label: 'IOTA CLI',
                link: {
                    type: 'doc',
                    id: 'references/cli',
                },
                items: [
                    'references/cli/client',
                    'references/cli/ptb',
                    'references/cli/console',
                    'references/cli/keytool',
                    'references/cli/move',
                    'references/cli/validator',
                    'references/cli/ceremony',
                    'references/cli/cheatsheet',
                ],
            },
            'references/rust-sdk',
            {
                type: 'link',
                label: 'Typescript SDK',
                href: '/references/ts-sdk/typescript',
            },
            'developer/iota-101/using-events',
        ],
    },
    {
        type: 'category',
        label: 'Move Overview',
        items: [
            'developer/iota-101/move-overview/move-overview',
            'developer/iota-101/move-overview/strings',
            'developer/iota-101/move-overview/collections',
            'developer/iota-101/move-overview/init',
            'developer/iota-101/move-overview/visibility',
            'developer/iota-101/move-overview/entry-functions',
            {
                type: 'category',
                label: 'Structs and Abilities',
                items: [
                    'developer/iota-101/move-overview/structs-and-abilities/struct',
                    'developer/iota-101/move-overview/structs-and-abilities/copy',
                    'developer/iota-101/move-overview/structs-and-abilities/drop',
                    'developer/iota-101/move-overview/structs-and-abilities/key',
                    'developer/iota-101/move-overview/structs-and-abilities/store',
                ],
            },
            'developer/iota-101/move-overview/one-time-witness',
            {
                type: 'category',
                label: 'Package Upgrades',
                items: [
                    'developer/iota-101/move-overview/package-upgrades/introduction',
                    'developer/iota-101/move-overview/package-upgrades/upgrade',
                    'developer/iota-101/move-overview/package-upgrades/automated-address-management',
                    'developer/iota-101/move-overview/package-upgrades/custom-policies',
                ],
            },
            'developer/iota-101/move-overview/ownership-scope',
            'developer/iota-101/move-overview/references',
            'developer/iota-101/move-overview/generics',
            {
                type: 'category',
                label: 'Patterns',
                items: [
                    'developer/iota-101/move-overview/patterns/patterns',
                    'developer/iota-101/move-overview/patterns/capabilities',
                    'developer/iota-101/move-overview/patterns/witness',
                    'developer/iota-101/move-overview/patterns/transferable-witness',
                    'developer/iota-101/move-overview/patterns/hot-potato',
                    'developer/iota-101/move-overview/patterns/id-pointer',
                ],
            },
            'developer/iota-101/access-time',
            'developer/iota-101/move-overview/conventions',
        ],
    },
    {
        type: 'category',
        label: 'Object Model',
        items: [
            'developer/iota-101/objects/object-model',
            'developer/iota-101/objects/shared-owned',
            {
                type: 'category',
                label: 'Object Ownership',
                link: {
                    type: 'doc',
                    id: 'developer/iota-101/objects/object-ownership/object-ownership',
                },
                items: [
                    'developer/iota-101/objects/object-ownership/address-owned',
                    'developer/iota-101/objects/object-ownership/immutable',
                    'developer/iota-101/objects/object-ownership/shared',
                    'developer/iota-101/objects/object-ownership/wrapped',
                ],
            },
            'developer/iota-101/objects/uid-id',
            {
                type: 'category',
                label: 'Dynamic Fields',
                link: {
                    type: 'doc',
                    id: 'developer/iota-101/objects/dynamic-fields/dynamic-fields',
                },
                items: ['developer/iota-101/objects/dynamic-fields/tables-bags'],
            },
            {
                type: 'category',
                label: 'Transfers',
                link: {
                    type: 'doc',
                    id: 'developer/iota-101/objects/transfers/transfers',
                },
                items: [
                    'developer/iota-101/objects/transfers/custom-rules',
                    'developer/iota-101/objects/transfers/transfer-to-object',
                ],
            },
            'developer/iota-101/objects/events',
            'developer/iota-101/objects/versioning',
        ],
    },
    {
        type: 'category',
        label: 'Transactions',
        link: {
            type: 'doc',
            id: 'developer/iota-101/transactions/transactions',
        },
        items: [
            'developer/iota-101/transactions/sign-and-send-transactions',
            {
                type: 'category',
                label: 'Sponsored Transactions',
                link: {
                    type: 'doc',
                    id: 'developer/iota-101/transactions/sponsored-transactions/about-sponsored-transactions',
                },
                items: [
                    'developer/iota-101/transactions/sponsored-transactions/about-sponsored-transactions',
                    'developer/iota-101/transactions/sponsored-transactions/use-sponsored-transactions',
                ],
            },
            {
                type: 'category',
                label: 'Working with PTBs',
                link: {
                    type: 'doc',
                    id: 'developer/iota-101/transactions/ptb/programmable-transaction-blocks-overview',
                },
                items: [
                    'developer/iota-101/transactions/ptb/programmable-transaction-blocks',
                    'developer/iota-101/transactions/ptb/building-programmable-transaction-blocks-ts-sdk',
                    'developer/iota-101/transactions/ptb/simulating-references',
                    'developer/iota-101/transactions/ptb/coin-management',
                    'developer/iota-101/transactions/ptb/optimizing-gas-with-coin-merging',
                ],
            },
        ],
    },
    {
        type: 'category',
        label: 'Create Coins and Tokens',
        link: {
            type: 'doc',
            id: 'developer/iota-101/create-coin/create-coin',
        },
        items: [
            'developer/iota-101/create-coin/regulated',
            'developer/iota-101/create-coin/migrate-to-coin-manager',
            'developer/iota-101/create-coin/in-game-token',
            'developer/iota-101/create-coin/loyalty',
        ],
    },
    {
        type: 'category',
        label: 'NFT',
        items: ['developer/iota-101/nft/create-nft', 'developer/iota-101/nft/rent-nft'],
    },
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
    {
        type: 'category',
        label: 'Cryptography',
        link: {
            type: 'doc',
            id: 'developer/cryptography',
        },
        items: [
            {
                type: 'category',
                label: 'Transaction Authentication',
                link: {
                    type: 'doc',
                    id: 'developer/cryptography/transaction-auth',
                },
                items: [
                    'developer/cryptography/transaction-auth/keys-addresses',
                    'developer/cryptography/transaction-auth/signatures',
                    'developer/cryptography/transaction-auth/multisig',
                    'developer/cryptography/transaction-auth/offline-signing',
                    'developer/cryptography/transaction-auth/intent-signing',
                ],
            },
            'developer/cryptography/checkpoint-verification',
            {
                type: 'category',
                label: 'Smart Contract Cryptography',
                link: {
                    type: 'doc',
                    id: 'developer/cryptography/on-chain',
                },
                items: [
                    'developer/cryptography/on-chain/signing',
                    'developer/cryptography/on-chain/groth16',
                    'developer/cryptography/on-chain/hashing',
                    'developer/cryptography/on-chain/ecvrf',
                ],
            },
        ],
    },
    {
        type: 'category',
        label: 'Standards',
        link: {
            type: 'generated-index',
            title: 'IOTA Standards Overview',
            description:
                'Standards on the IOTA blockchain are features, frameworks, or apps that you can extend or customize.',
            slug: 'developer/standards',
        },
        items: [
            'developer/standards/coin',
            'developer/standards/coin-manager',
            {
                type: 'category',
                label: 'Closed-Loop Token',
                link: {
                    type: 'doc',
                    id: 'developer/standards/closed-loop-token',
                },
                items: [
                    'developer/standards/closed-loop-token/action-request',
                    'developer/standards/closed-loop-token/token-policy',
                    'developer/standards/closed-loop-token/spending',
                    'developer/standards/closed-loop-token/rules',
                    'developer/standards/closed-loop-token/coin-token-comparison',
                ],
            },
            'developer/standards/kiosk',
            'developer/standards/kiosk-apps',
            'developer/standards/display',
            'developer/standards/wallet-standard',
        ],
    },
    {
        type: 'category',
        label: 'Capture The Flag',
        link: {
            type: 'doc',
            id: 'developer/iota-move-ctf/introduction',
        },
        items: [
            'developer/iota-move-ctf/challenge_1',
            'developer/iota-move-ctf/challenge_2',
            'developer/iota-move-ctf/challenge_3',
            'developer/iota-move-ctf/challenge_4',
            'developer/iota-move-ctf/challenge_5',
            'developer/iota-move-ctf/challenge_6',
            'developer/iota-move-ctf/challenge_7',
            'developer/iota-move-ctf/challenge_8',
        ],
    },
    {
        type: 'category',
        label: 'From Solidity/EVM to Move',
        collapsed: true,
        link: {
            type: 'doc',
            id: 'developer/evm-to-move/evm-to-move',
        },
        items: [
            'developer/evm-to-move/tooling-apis',
            'developer/evm-to-move/creating-token',
            'developer/evm-to-move/creating-nft',
        ],
    },
    {
        type: 'category',
        label: 'GraphQL',
        items: [
            'developer/getting-started/graphql-rpc',
            'developer/graphql-rpc',
            'developer/advanced/graphql-migration',
        ],
    },
    {
        type: 'category',
        label: 'Advanced Topics',
        link: {
            type: 'doc',
            id: 'developer/advanced',
        },
        items: [
            'developer/advanced/introducing-move-2024',
            'developer/advanced/iota-repository',
            'developer/advanced/custom-indexer',
            'developer/advanced/onchain-randomness',
            'developer/advanced/asset-tokenization',
            'developer/advanced/create-review-rating-dao-with-multisig',
        ],
    },
    {
        type: 'category',
        label: 'Migrating from IOTA Stardust',
        link: {
            type: 'doc',
            id: 'developer/stardust/stardust-migration',
        },
        items: [
            'developer/stardust/exchanges',
            'developer/stardust/move-models',
            'developer/stardust/addresses',
            'developer/stardust/units',
            'developer/stardust/migration-process',
            {
                type: 'category',
                label: 'Claiming Stardust Assets',
                link: {
                    type: 'doc',
                    id: 'developer/stardust/claiming',
                },
                items: [
                    {
                        type: 'doc',
                        label: 'Basic Outputs',
                        id: 'developer/stardust/claiming/basic',
                    },
                    {
                        type: 'doc',
                        label: 'Nft Outputs',
                        id: 'developer/stardust/claiming/nft',
                    },
                    {
                        type: 'doc',
                        label: 'Alias Outputs',
                        id: 'developer/stardust/claiming/alias',
                    },
                    {
                        type: 'doc',
                        label: 'Foundry Outputs',
                        id: 'developer/stardust/claiming/foundry',
                    },
                    {
                        type: 'doc',
                        label: 'Output unlockable by an Alias/Nft Address',
                        id: 'developer/stardust/claiming/address-unlock-condition',
                    },
                    {
                        type: 'doc',
                        label: 'Self-sponsor Iota Claiming',
                        id: 'developer/stardust/claiming/self-sponsor',
                    },
                ],
            },
        ],
    },
    {
        type: 'category',
        label: 'Exchange integration',
        items: ['developer/exchange-integration/exchange-integration'],
    },
    'developer/dev-cheat-sheet',
];
module.exports = developer;
