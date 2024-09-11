// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

const references = [
    {
        type: 'doc',
        label: 'References',
        id: 'references/references',
    },
    {
        type: 'category',
        label: 'IOTA RPC',
        collapsed: false,
        link: {
            type: 'doc',
            id: 'references/iota-api',
        },
        items: [
            /*{
				type: 'category',
				label: 'GraphQL',
				link: {
					type: 'doc',
					id: 'references/iota-graphql',
				},
				items: [
					{
						type: 'autogenerated',
						dirName: 'references/iota-api/iota-graphql/reference',
					},
				],
			},*/
            {
                type: 'link',
                label: 'JSON-RPC',
                href: '/iota-api-ref',
                description: 'IOTA JSON-RPC API Reference',
            },
            'references/iota-api/rpc-best-practices',
        ],
    },
    {
        type: 'category',
        label: 'IOTA CLI',
        collapsed: false,
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
        ],
    },
    {
        type: 'category',
        label: 'IOTA SDKs',
        collapsed: false,
        link: {
            type: 'doc',
            id: 'references/iota-sdks',
        },
        items: [
            'references/rust-sdk',
            {
                type: 'category',
                label: 'IOTA TypeScript Documentation',
                items: [
                    {
                        type: 'category',
                        label: 'Typescript SDK',
                        items: [
                            'references/ts-sdk/typescript/index',
                            'references/ts-sdk/typescript/install',
                            'references/ts-sdk/typescript/hello-iota',
                            'references/ts-sdk/typescript/faucet',
                            'references/ts-sdk/typescript/iota-client',
                            {
                                type: 'category',
                                label: 'Transaction Building',
                                items: [
                                    'references/ts-sdk/typescript/transaction-building/basics',
                                    'references/ts-sdk/typescript/transaction-building/gas',
                                    'references/ts-sdk/typescript/transaction-building/sponsored-transactions',
                                    'references/ts-sdk/typescript/transaction-building/offline',
                                ],
                            },
                            {
                                type: 'category',
                                label: 'Cryptography',
                                items: [
                                    'references/ts-sdk/typescript/cryptography/keypairs',
                                    'references/ts-sdk/typescript/cryptography/multisig',
                                ],
                            },
                            'references/ts-sdk/typescript/utils',
                            'references/ts-sdk/typescript/bcs',
                            {
                                type: 'category',
                                label: 'Owned Object Pool',
                                items: [
                                    'references/ts-sdk/typescript/owned-object-pool/index',
                                    'references/ts-sdk/typescript/owned-object-pool/overview',
                                    'references/ts-sdk/typescript/owned-object-pool/local-development',
                                    'references/ts-sdk/typescript/owned-object-pool/custom-split-strategy',
                                    'references/ts-sdk/typescript/owned-object-pool/examples',
                                ],
                            },
                        ],
                    },
                    {
                        type: 'category',
                        label: 'dApp Kit',
                        items: [
                            'references/ts-sdk/dapp-kit/index',
                            'references/ts-sdk/dapp-kit/create-dapp',
                            'references/ts-sdk/dapp-kit/iota-client-provider',
                            'references/ts-sdk/dapp-kit/rpc-hooks',
                            'references/ts-sdk/dapp-kit/wallet-provider',
                            {
                                type: 'category',
                                label: 'Wallet Components',
                                items: [
                                    'references/ts-sdk/dapp-kit/wallet-components/ConnectButton',
                                    'references/ts-sdk/dapp-kit/wallet-components/ConnectModal',
                                ],
                            },
                            {
                                type: 'category',
                                label: 'Wallet Hooks',
                                items: [
                                    'references/ts-sdk/dapp-kit/wallet-hooks/useWallets',
                                    'references/ts-sdk/dapp-kit/wallet-hooks/useAccounts',
                                    'references/ts-sdk/dapp-kit/wallet-hooks/useCurrentWallet',
                                    'references/ts-sdk/dapp-kit/wallet-hooks/useCurrentAccount',
                                    'references/ts-sdk/dapp-kit/wallet-hooks/useAutoConnectWallet',
                                    'references/ts-sdk/dapp-kit/wallet-hooks/useConnectWallet',
                                    'references/ts-sdk/dapp-kit/wallet-hooks/useDisconnectWallet',
                                    'references/ts-sdk/dapp-kit/wallet-hooks/useSwitchAccount',
                                    'references/ts-sdk/dapp-kit/wallet-hooks/useSignPersonalMessage',
                                    'references/ts-sdk/dapp-kit/wallet-hooks/useSignTransactionBlock',
                                    'references/ts-sdk/dapp-kit/wallet-hooks/useSignAndExecuteTransactionBlock',
                                ],
                            },
                            'references/ts-sdk/dapp-kit/themes',
                        ],
                    },
                    {
                        type: 'category',
                        label: 'Kiosk SDK',
                        items: [
                            'references/ts-sdk/kiosk/index',
                            {
                                type: 'category',
                                label: 'Kiosk Client',
                                items: [
                                    'references/ts-sdk/kiosk/kiosk-client/introduction',
                                    'references/ts-sdk/kiosk/kiosk-client/querying',
                                    {
                                        type: 'category',
                                        label: 'Kiosk Transactions',
                                        items: [
                                            'references/ts-sdk/kiosk/kiosk-client/kiosk-transaction/kiosk-transaction',
                                            'references/ts-sdk/kiosk/kiosk-client/kiosk-transaction/managing',
                                            'references/ts-sdk/kiosk/kiosk-client/kiosk-transaction/purchasing',
                                            'references/ts-sdk/kiosk/kiosk-client/kiosk-transaction/examples',
                                        ],
                                    },
                                    {
                                        type: 'category',
                                        label: 'Transfer Policy Transactions',
                                        items: [
                                            'references/ts-sdk/kiosk/kiosk-client/transfer-policy-transaction/introduction',
                                            'references/ts-sdk/kiosk/kiosk-client/transfer-policy-transaction/using-the-manager',
                                        ],
                                    },
                                ],
                            },
                            'references/ts-sdk/kiosk/advanced-examples',
                            'references/ts-sdk/kiosk/from-v1',
                        ],
                    },
                    'references/ts-sdk/bcs',
                ],
            },
        ],
    },
    {
        type: 'category',
        label: 'Move',
        collapsed: false,
        link: {
            type: 'doc',
            id: 'references/iota-move',
        },
        items: [
            {
                type: 'category',
                label: 'Framework',
                link: {
                    type: 'doc',
                    id: 'references/framework',
                },
                items: [{ type: 'autogenerated', dirName: 'references/framework' }],
            },
            'references/move/move-toml',
            'references/move/move-lock',
            {
                type: 'link',
                label: 'Move Language (GitHub)',
                href: 'https://github.com/move-language/move/blob/main/language/documentation/book/src/introduction.md',
                description: 'Move Language Documentation (GitHub Repository)',
            },
        ],
    },
    {
        type: 'category',
        label: 'IOTA EVM',
        items: [
            'references/iota-evm/json-rpc-spec',
            {
                type: 'category',
                label: 'Magic Contract',
                items: [
                    {
                        type: 'autogenerated',
                        dirName: 'references/iota-evm/magic-contract',
                    },
                ],
            },
            {
                type: 'category',
                label: 'Core Contracts',
                items: [
                    {
                        type: 'doc',
                        label: 'Overview',
                        id: 'references/iota-evm/core-contracts/overview',
                    },
                    {
                        type: 'doc',
                        label: 'root',
                        id: 'references/iota-evm/core-contracts/root',
                    },
                    {
                        type: 'doc',
                        label: 'accounts',
                        id: 'references/iota-evm/core-contracts/accounts',
                    },
                    {
                        type: 'doc',
                        label: 'blob',
                        id: 'references/iota-evm/core-contracts/blob',
                    },
                    {
                        type: 'doc',
                        label: 'blocklog',
                        id: 'references/iota-evm/core-contracts/blocklog',
                    },
                    {
                        type: 'doc',
                        label: 'governance',
                        id: 'references/iota-evm/core-contracts/governance',
                    },
                    {
                        type: 'doc',
                        label: 'errors',
                        id: 'references/iota-evm/core-contracts/errors',
                    },
                    {
                        type: 'doc',
                        label: 'EVM',
                        id: 'references/iota-evm/core-contracts/evm',
                    },
                ],
            },
            {
                type: 'category',
                label: 'ISC Utilities',
                items: [
                    {
                        type: 'autogenerated',
                        dirName: 'references/iota-evm/iscutils',
                    },
                ],
            },
            {
                type: 'doc',
                label: 'WasmLib Data Types',
                id: 'references/iota-evm/wasm-lib-data-types',
            },
        ],
    },
    {
        type: 'category',
        label: 'IOTA Identity',
        link: {
            type: 'doc',
            id: 'references/iota-identity/overview',
        },
        items: [
            'references/iota-identity/overview',
            'references/iota-identity/iota-did-method-spec',
            'references/iota-identity/revocation-bitmap-2022',
            'references/iota-identity/revocation-timeframe-2024',
            {
                type: 'category',
                label: 'Wasm',
                items: [
                    {
                        type: 'autogenerated',
                        dirName: 'references/iota-identity/wasm',
                    },
                ],
            },
            {
                type: 'link',
                label: 'Rust',
                href: 'https://docs.rs/identity_iota/latest/identity_iota/index.html',
                description: 'IOTA Identity Rust Documentation',
            },
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
                    id: 'references/execution-architecture/execution-layer',
                },
                items: [
                    'references/execution-architecture/iota-execution',
                    'references/execution-architecture/adapter',
                    'references/execution-architecture/natives',
                ],
            },
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
                    id: 'references/execution-architecture/execution-layer',
                },
                items: [
                    'references/execution-architecture/iota-execution',
                    'references/execution-architecture/adapter',
                    'references/execution-architecture/natives',
                ],
            },
        ],
    },
    'references/research-papers',
    'references/iota-glossary',
    {
        type: 'category',
        label: 'Contribute',
        link: {
            type: 'doc',
            id: 'references/contribute/contribution-process',
        },
        items: [
            'references/contribute/contribution-process',
            'references/contribute/code-of-conduct',
            'references/contribute/style-guide',
            'references/contribute/add-a-quiz',
            'references/contribute/import-code-docs'
        ],
    },
];

module.exports = references;
