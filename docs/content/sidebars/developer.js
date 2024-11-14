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
            'developer/getting-started/graphql-rpc',
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
        label: 'Move Overview',
        items: [
            'developer/move-overview/move-overview',
            'developer/move-overview/strings',
            'developer/move-overview/collections',
            'developer/move-overview/init',
            'developer/move-overview/visibility',
            'developer/move-overview/entry-functions',
            {
                type: 'category',
                label: 'Structs and Abilities',
                items: [
                    'developer/move-overview/structs-and-abilities/struct',
                    'developer/move-overview/structs-and-abilities/copy',
                    'developer/move-overview/structs-and-abilities/drop',
                    'developer/move-overview/structs-and-abilities/key',
                    'developer/move-overview/structs-and-abilities/store',
                ],
            },
            'developer/move-overview/one-time-witness',
            {
                type: 'category',
                label: 'Package Upgrades',
                items: [
                    'developer/move-overview/package-upgrades/introduction',
                    'developer/move-overview/package-upgrades/upgrade',
                    'developer/move-overview/package-upgrades/automated-address-management',
                    'developer/move-overview/package-upgrades/custom-policies',
                ],
            },
            'developer/move-overview/ownership-scope',
            'developer/move-overview/references',
            'developer/move-overview/generics',
            {
                type: 'category',
                label: 'Patterns',
                items: [
                    'developer/move-overview/patterns/patterns',
                    'developer/move-overview/patterns/capabilities',
                    'developer/move-overview/patterns/witness',
                    'developer/move-overview/patterns/transferable-witness',
                    'developer/move-overview/patterns/hot-potato',
                    'developer/move-overview/patterns/id-pointer',
                ],
            },
            'developer/move-overview/conventions',
        ],
    },
    'developer/graphql-rpc',
    {
        type: 'category',
        label: 'Object Model',
        items: [
            'developer/objects/object-model',
            'developer/objects/shared-owned',
            {
                type: 'category',
                label: 'Object Ownership',
                link: {
                    type: 'doc',
                    id: 'developer/objects/object-ownership/object-ownership',
                },
                items: [
                    'developer/objects/object-ownership/address-owned',
                    'developer/objects/object-ownership/immutable',
                    'developer/objects/object-ownership/shared',
                    'developer/objects/object-ownership/wrapped',
                ],
            },
            'developer/objects/uid-id',
            {
                type: 'category',
                label: 'Dynamic Fields',
                link: {
                    type: 'doc',
                    id: 'developer/objects/dynamic-fields/dynamic-fields',
                },
                items: ['developer/objects/dynamic-fields/tables-bags'],
            },
            {
                type: 'category',
                label: 'Transfers',
                link: {
                    type: 'doc',
                    id: 'developer/objects/transfers/transfers',
                },
                items: [
                    'developer/objects/transfers/custom-rules',
                    'developer/objects/transfers/transfer-to-object',
                ],
            },
            'developer/objects/events',
            'developer/objects/versioning',
        ],
    },
    {
        type: 'category',
        label: 'Transactions',
        link: {
            type: 'doc',
            id: 'developer/transactions/transactions',
        },
        items: [
            'developer/transactions/sign-and-send-transactions',
            {
                type: 'category',
                label: 'Sponsored Transactions',
                link: {
                    type: 'doc',
                    id: 'developer/transactions/sponsored-transactions/about-sponsored-transactions',
                },
                items: [
                    'developer/transactions/sponsored-transactions/about-sponsored-transactions',
                    'developer/transactions/sponsored-transactions/use-sponsored-transactions'],
            },
            {
                type: 'category',
                label: 'Working with PTBs',
                link: {
                    type: 'doc',
                    id:'developer/transactions/ptb/programmable-transaction-blocks-overview',
                },
                items: [
                    'developer/transactions/ptb/programmable-transaction-blocks',
                    'developer/transactions/ptb/building-programmable-transaction-blocks-ts-sdk',
                    'developer/transactions/ptb/simulating-references',
                    'developer/transactions/ptb/coin-management',
                    'developer/transactions/ptb/optimizing-gas-with-coin-merging',

                ],
            },
        ],
    },
    {
        type: 'category',
        label: 'Create Coins and Tokens',
        link: {
            type: 'doc',
            id: 'developer/create-coin/create-coin',
        },
        items: [
            'developer/create-coin/regulated',
            'developer/create-coin/migrate-to-coin-manager',
            'developer/create-coin/in-game-token',
            'developer/create-coin/loyalty',
        ],
    },
    {
        type: 'category',
        label: 'NFT',
        items: ['developer/nft/create-nft', 'developer/nft/rent-nft'],
    },
    'developer/using-events',
    'developer/access-time',
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
            title:'IOTA Standards Overview',
            description: 'Standards on the IOTA blockchain are features, frameworks, or apps that you can extend or customize.',
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
        label: 'Advanced Topics',
        link: {
            type: 'doc',
            id: 'developer/advanced',
        },
        items: [
            'developer/advanced/introducing-move-2024',
            'developer/advanced/iota-repository',
            'developer/advanced/custom-indexer',
            'developer/advanced/graphql-migration',
            'developer/advanced/onchain-randomness',
            'developer/advanced/asset-tokenization',
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
