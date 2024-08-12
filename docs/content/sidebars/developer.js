// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

const developer = [
    'developer/developer',
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
            'developer/getting-started/iota-install',
            'developer/getting-started/connect',
            'developer/getting-started/local-network',
            'developer/getting-started/get-address',
            'developer/getting-started/get-coins',
            //'developer/getting-started/graphql-rpc',
            {
                type: 'category',
                label: 'Your First IOTA dApp',
                collapsed: false,
                link: {
                    type: 'doc',
                    id: 'developer/getting-started/first-app/first-app',
                },
                items: [
                    'developer/getting-started/first-app/write-package',
                    'developer/getting-started/first-app/build-test',
                    'developer/getting-started/first-app/publish',
                    'developer/getting-started/first-app/debug',
                    'developer/getting-started/first-app/client-tssdk',
                ],
            },
        ],
    },
    {
        type: 'category',
        label: 'IOTA 101',
        link: {
            type: 'doc',
            id: 'developer/iota-101',
        },
        items: [
            {
                type: 'category',
                label: 'Move Overview',
                link: {
                    type: 'doc',
                    id: 'developer/iota-101/iota-move-concepts/iota-move-concepts',
                },
                items: [
                    'developer/iota-101/iota-move-concepts/strings',
                    'developer/iota-101/iota-move-concepts/collections',
                    'developer/iota-101/iota-move-concepts/init',
                    'developer/iota-101/iota-move-concepts/entry-functions',
                    'developer/iota-101/iota-move-concepts/one-time-witness',
                    {
                        type: 'category',
                        label: 'Package Upgrades',
                        link: {
                            type: 'doc',
                            id: 'developer/iota-101/iota-move-concepts/packages/packages',
                        },
                        items: [
                            'developer/iota-101/iota-move-concepts/packages/upgrade',
                            'developer/iota-101/iota-move-concepts/packages/custom-policies',
                        ],
                    },
                    {
                        type: 'category',
                        label: 'Patterns',
                        link: {
                            type: 'doc',
                            id: 'developer/iota-101/iota-move-concepts/patterns',
                        },
                        items: [
                            'developer/iota-101/iota-move-concepts/patterns/capabilities',
                            'developer/iota-101/iota-move-concepts/patterns/witness',
                            'developer/iota-101/iota-move-concepts/patterns/transferrable-witness',
                            'developer/iota-101/iota-move-concepts/patterns/hot-potato',
                            'developer/iota-101/iota-move-concepts/patterns/id-pointer',
                        ],
                    },
                    'developer/iota-101/iota-move-concepts/conventions',
                ],
            },
            //'developer/graphql-rpc',
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
                    'developer/iota-101/transactions/sign-and-send-txn',
                    {
                        type: 'category',
                        label: 'Sponsored Transactions',
                        link: {
                            type: 'doc',
                            id: 'developer/iota-101/transactions/sponsor-txn',
                        },
                        items: ['developer/iota-101/transactions/sponsored-transactions'],
                    },
                    'developer/iota-101/transactions/gas-smashing',
                    {
                        type: 'category',
                        label: 'Working with PTBs',
                        items: [
                            'developer/iota-101/transactions/ptb/prog-txn-blocks',
                            'developer/iota-101/transactions/ptb/building-ptb',
                            'developer/iota-101/transactions/ptb/coin-mgt',
                            'developer/iota-101/transactions/ptb/simulating-refs',
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
                    'developer/iota-101/create-coin/in-game-token',
                    'developer/iota-101/create-coin/loyalty',
                ],
            },
            'developer/iota-101/create-nft',
            'developer/iota-101/using-events',
            'developer/iota-101/access-time',
        ],
    },
    {
        type: 'category',
        label: 'Standards',
        items: [
            'developer/standards/standards',
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
        label: 'From Solidity/EVM to Move',
        collapsed: true,
        link: {
            type: 'doc',
            id: 'developer/evm-to-move/evm-to-move',
        },
        items: [
            'developer/evm-to-move/why-move',
            'developer/evm-to-move/tooling-apis',
            'developer/evm-to-move/creating-token',
            'developer/evm-to-move/creating-nft',
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
        label: 'Advanced Topics',
        link: {
            type: 'doc',
            id: 'developer/advanced',
        },
        items: [
            'developer/advanced/asset-tokenization',
        ],
    },

    {
        type: 'category',
        label: 'Solidity/EVM Smart Contracts',
        link: {
            type: 'doc',
            id: 'developer/iota-evm/introduction',
        },
        items: [
            {
                type: 'doc',
                label: 'Introduction',
                id: 'developer/iota-evm/introduction',
            },
            {
                type: 'category',
                label: 'Getting Started',
                items: [
                    {
                        type: 'doc',
                        label: 'Languages & VMs',
                        id: 'developer/iota-evm/getting-started/languages-and-vms',
                    },
                    'developer/iota-evm/getting-started/quick-start',
                    'developer/iota-evm/getting-started/compatibility',
                    {
                        type: 'doc',
                        label: 'Networks & Chains',
                        id: 'developer/iota-evm/getting-started/networks-and-chains',
                    },
                    {
                        type: 'doc',
                        label: 'Tools',
                        id: 'developer/iota-evm/getting-started/tools',
                    },
                ],
            },
            {
                type: 'category',
                label: 'How To',
                items: [
                    'developer/iota-evm/how-tos/introduction',
                    {
                        type: 'doc',
                        label: 'Send Funds from L1 to L2',
                        id: 'developer/iota-evm/how-tos/send-funds-from-L1-to-L2',
                    },
                    {
                        type: 'doc',
                        label: 'Create a Basic Contract',
                        id: 'developer/iota-evm/how-tos/create-a-basic-contract',
                    },
                    {
                        type: 'doc',
                        label: 'Deploy a Smart Contract',
                        id: 'developer/iota-evm/how-tos/deploy-a-smart-contract',
                    },
                    {
                        type: 'doc',
                        label: 'Create Custom Tokens - ERC20',
                        id: 'developer/iota-evm/how-tos/ERC20',
                    },
                    {
                        type: 'doc',
                        label: 'Send ERC20 Tokens Across Chains',
                        id: 'developer/iota-evm/how-tos/send-ERC20-across-chains',
                    },
                    {
                        type: 'doc',
                        label: 'Create NFTs - ERC721',
                        id: 'developer/iota-evm/how-tos/ERC721',
                    },
                    {
                        type: 'doc',
                        label: 'Send NFTs Across Chains',
                        id: 'developer/iota-evm/how-tos/send-NFTs-across-chains',
                    },
                    {
                        type: 'doc',
                        label: 'Test Smart Contracts',
                        id: 'developer/iota-evm/how-tos/test-smart-contracts',
                    },
                    {
                        type: 'category',
                        label: 'Interact with the Core Contracts',
                        items: [
                            {
                                type: 'doc',
                                label: 'Introduction',
                                id: 'developer/iota-evm/how-tos/core-contracts/introduction',
                            },
                            {
                                type: 'category',
                                label: 'Basics',
                                items: [
                                    {
                                        type: 'doc',
                                        label: 'Get Native Assets Balance',
                                        id: 'developer/iota-evm/how-tos/core-contracts/basics/get-balance',
                                    },
                                    {
                                        type: 'category',
                                        label: 'Allowance',
                                        items: [
                                            {
                                                type: 'doc',
                                                label: 'Allow',
                                                id: 'developer/iota-evm/how-tos/core-contracts/basics/allowance/allow',
                                            },
                                            {
                                                type: 'doc',
                                                label: 'Get Allowance',
                                                id: 'developer/iota-evm/how-tos/core-contracts/basics/allowance/get-allowance',
                                            },
                                            {
                                                type: 'doc',
                                                label: 'Take Allowance',
                                                id: 'developer/iota-evm/how-tos/core-contracts/basics/allowance/take-allowance',
                                            },
                                        ],
                                    },
                                    {
                                        type: 'doc',
                                        label: 'Send Assets to L1',
                                        id: 'developer/iota-evm/how-tos/core-contracts/basics/send-assets-to-l1',
                                    },
                                ],
                            },
                            {
                                type: 'category',
                                label: 'Token',
                                items: [
                                    {
                                        label: 'Introduction',
                                        type: 'doc',
                                        id: 'developer/iota-evm/how-tos/core-contracts/token/introduction',
                                    },
                                    {
                                        type: 'doc',
                                        label: 'Create a Native Token',
                                        id: 'developer/iota-evm/how-tos/core-contracts/token/create-native-token',
                                    },
                                    {
                                        type: 'doc',
                                        label: 'Mint Native Tokens',
                                        id: 'developer/iota-evm/how-tos/core-contracts/token/mint-token',
                                    },
                                    {
                                        type: 'doc',
                                        label: 'Custom ERC20 Functions',
                                        id: 'developer/iota-evm/how-tos/core-contracts/token/erc20-native-token',
                                    },
                                    {
                                        type: 'doc',
                                        label: 'Create a Foundry',
                                        id: 'developer/iota-evm/how-tos/core-contracts/token/create-foundry',
                                    },
                                    {
                                        type: 'doc',
                                        label: 'Register Token as ERC20',
                                        id: 'developer/iota-evm/how-tos/core-contracts/token/register-token',
                                    },
                                ],
                            },
                            {
                                type: 'category',
                                label: 'NFT',
                                items: [
                                    {
                                        label: 'Introduction',
                                        type: 'doc',
                                        id: 'developer/iota-evm/how-tos/core-contracts/nft/introduction',
                                    },
                                    {
                                        type: 'doc',
                                        label: 'Mint an NFT',
                                        id: 'developer/iota-evm/how-tos/core-contracts/nft/mint-nft',
                                    },
                                    {
                                        type: 'doc',
                                        label: 'Use as ERC721',
                                        id: 'developer/iota-evm/how-tos/core-contracts/nft/use-as-erc721',
                                    },
                                ],
                            },
                            {
                                type: 'doc',
                                label: 'Get Randomness on L2',
                                id: 'developer/iota-evm/how-tos/core-contracts/get-randomness-on-l2',
                            },
                            {
                                type: 'doc',
                                label: 'Call and Call View',
                                id: 'developer/iota-evm/how-tos/core-contracts/call-view',
                            },
                        ],
                    },
                ],
            },
            {
                type: 'category',
                label: 'Explanations',
                items: [
                    {
                        type: 'doc',
                        label: 'Anatomy of a Smart Contract',
                        id: 'developer/iota-evm/explanations/smart-contract-anatomy',
                    },
                    {
                        type: 'doc',
                        label: 'Sandbox Interface',
                        id: 'developer/iota-evm/explanations/sandbox',
                    },
                    {
                        type: 'doc',
                        label: 'Calling a Smart Contract',
                        id: 'developer/iota-evm/explanations/invocation',
                    },
                    {
                        type: 'doc',
                        label: 'State, Transitions and State Anchoring',
                        id: 'developer/iota-evm/explanations/states',
                    },
                    {
                        type: 'doc',
                        label: 'State manager',
                        id: 'developer/iota-evm/explanations/state_manager',
                    },
                    {
                        type: 'doc',
                        label: 'Validators and Access Nodes',
                        id: 'developer/iota-evm/explanations/validators',
                    },
                    {
                        type: 'doc',
                        label: 'Consensus',
                        id: 'developer/iota-evm/explanations/consensus',
                    },
                    {
                        type: 'doc',
                        label: 'How Accounts Work',
                        id: 'developer/iota-evm/explanations/how-accounts-work',
                    },
                    {
                        type: 'doc',
                        label: 'Core Contracts',
                        id: 'references/iota-evm/core-contracts/overview',
                    },
                ],
            },
            {
                type: 'category',
                label: 'Test with Solo',
                items: [
                    {
                        label: 'Getting Started',
                        id: 'developer/iota-evm/solo/getting-started',
                        type: 'doc',
                    },
                    {
                        type: 'category',
                        label: 'How To',
                        items: [
                            {
                                type: 'doc',
                                label: 'First Example',
                                id: 'developer/iota-evm/solo/how-tos/first-example',
                            },
                            {
                                type: 'doc',
                                label: 'The L1 Ledger',
                                id: 'developer/iota-evm/solo/how-tos/the-l1-ledger',
                            },
                            {
                                type: 'doc',
                                label: 'Deploy a Smart Contract',
                                id: 'developer/iota-evm/solo/how-tos/deploying-sc',
                            },
                            {
                                type: 'doc',
                                label: 'Invoke a Smart Contract',
                                id: 'developer/iota-evm/solo/how-tos/invoking-sc',
                            },
                            {
                                type: 'doc',
                                label: 'Call a View',
                                id: 'developer/iota-evm/solo/how-tos/view-sc',
                            },
                            {
                                type: 'doc',
                                label: 'Error Handling',
                                id: 'developer/iota-evm/solo/how-tos/error-handling',
                            },
                            {
                                type: 'doc',
                                label: 'Accounts',
                                id: 'developer/iota-evm/solo/how-tos/the-l2-ledger',
                            },
                            {
                                type: 'doc',
                                label: 'Test Smart Contracts',
                                id: 'developer/iota-evm/solo/how-tos/test',
                            },
                            {
                                type: 'doc',
                                label: 'Example Tests',
                                id: 'developer/iota-evm/solo/how-tos/examples',
                            },
                            {
                                type: 'doc',
                                label: 'Colored Tokens and Time Locks',
                                id: 'developer/iota-evm/solo/how-tos/timelock',
                            },
                        ],
                    },
                ],
            },
            {
                type: 'category',
                label: 'Wasm - Schema Tool',
                items: [
                    {
                        type: 'doc',
                        label: 'The Schema Tool',
                        id: 'developer/iota-evm/schema/introduction',
                    },
                    {
                        type: 'doc',
                        label: 'Data Access Proxies',
                        id: 'developer/iota-evm/schema/proxies',
                    },
                    {
                        type: 'category',
                        label: 'How To',
                        items: [
                            {
                                type: 'doc',
                                label: 'Create a Schema',
                                id: 'developer/iota-evm/schema/how-tos/usage',
                            },
                            {
                                type: 'doc',
                                label: 'Define the State',
                                id: 'developer/iota-evm/schema/how-tos/state',
                            },
                            {
                                type: 'doc',
                                label: 'Use Structured Data Types',
                                id: 'developer/iota-evm/schema/how-tos/structs',
                            },
                            {
                                type: 'doc',
                                label: 'Generate Type Definitions',
                                id: 'developer/iota-evm/schema/how-tos/typedefs',
                            },
                            {
                                type: 'doc',
                                label: 'Trigger Events',
                                id: 'developer/iota-evm/schema/how-tos/events',
                            },
                            {
                                type: 'doc',
                                label: 'Define Functions',
                                id: 'developer/iota-evm/schema/how-tos/funcs',
                            },
                            {
                                type: 'doc',
                                label: 'Limit Access',
                                id: 'developer/iota-evm/schema/how-tos/access',
                            },
                            {
                                type: 'doc',
                                label: 'Define Function Parameters',
                                id: 'developer/iota-evm/schema/how-tos/params',
                            },
                            {
                                type: 'doc',
                                label: 'Define Function Results',
                                id: 'developer/iota-evm/schema/how-tos/results',
                            },
                            {
                                type: 'doc',
                                label: 'Use Thunk Functions',
                                id: 'developer/iota-evm/schema/how-tos/thunks',
                            },
                            {
                                type: 'doc',
                                label: 'Use View-Only Functions',
                                id: 'developer/iota-evm/schema/how-tos/views',
                            },
                            {
                                type: 'doc',
                                label: 'Initialize a Smart Contract',
                                id: 'developer/iota-evm/schema/how-tos/init',
                            },
                            {
                                type: 'doc',
                                label: 'Transfer Tokens',
                                id: 'developer/iota-evm/schema/how-tos/transfers',
                            },
                            {
                                type: 'doc',
                                label: 'Add Function Descriptors',
                                id: 'developer/iota-evm/schema/how-tos/funcdesc',
                            },
                            {
                                type: 'doc',
                                label: 'Call Functions',
                                id: 'developer/iota-evm/schema/how-tos/call',
                            },
                            {
                                type: 'doc',
                                label: 'Post Asynchronous Requests',
                                id: 'developer/iota-evm/schema/how-tos/post',
                            },
                        ],
                    },
                ],
            },
        ],
    },

    {
        type: 'category',
        label: 'Migrating from IOTA/Shimmer Stardust',
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
            'developer/stardust/claiming',
            'developer/stardust/vested',
            'developer/stardust/testing',
            'developer/stardust/if-tools',
            'developer/stardust/faq',
            'developer/stardust/advanced',
        ],
    },

    {
        type: 'category',
        label: 'Exchange integration',
        items: ['developer/exchange-integration/exchange-integration'],
    },
    {
        type: 'category',
        label: 'Decentralized Identity',
        link: {
            type: 'doc',
            id: 'developer/iota-identity/welcome',
        },
        items: [
            {
                type: 'doc',
                id: 'developer/iota-identity/welcome',
                label: 'Welcome',
            },
            {
                type: 'category',
                label: 'Getting Started',
                collapsed: false,
                items: [
                    'developer/iota-identity/getting-started/rust',
                    'developer/iota-identity/getting-started/wasm',
                ],
            },
            {
                type: 'category',
                label: 'Explanations',
                items: [
                    'developer/iota-identity/explanations/decentralized-identifiers',
                    'developer/iota-identity/explanations/verifiable-credentials',
                    'developer/iota-identity/explanations/verifiable-presentations',
                    'developer/iota-identity/explanations/about-alias-outputs',
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
                        items: [
                            'developer/iota-identity/how-tos/verifiable-presentations/create-and-validate',
                        ],
                    },
                    {
                        type: 'category',
                        label: 'Domain Linkage',
                        items: ['developer/iota-identity/how-tos/domain-linkage/create-and-verify'],
                    },
                    'developer/iota-identity/how-tos/key-storage',
                ],
            },
            'developer/iota-identity/glossary',
            'developer/iota-identity/contribute',
            'developer/iota-identity/workflow',
            'developer/iota-identity/contact',
            'developer/iota-identity/faq',
        ],
    },
    'developer/dev-cheat-sheet',
];
module.exports = developer;
