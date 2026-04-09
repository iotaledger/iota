// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import typedocSidebarIOTASdk from '../developer/ts-sdk/typescript/api/typedoc-sidebar.cjs';
import typedocSidebarDappKit from '../developer/ts-sdk/dapp-kit/api/typedoc-sidebar.cjs';
import typedocSidebarKiosk from '../developer/ts-sdk/kiosk/api/typedoc-sidebar.cjs';
import typedocSidebarGraphqlTransport from '../developer/ts-sdk/graphql-transport/api/typedoc-sidebar.cjs';
import typedocSidebarWalletStandard from '../developer/ts-sdk/wallet-standard/api/typedoc-sidebar.cjs';
import typedocSidebarLedger from '../developer/ts-sdk/ledgerjs-hw-app-iota/api/typedoc-sidebar.cjs';
import typedocSidebarSigners from '../developer/ts-sdk/signers/api/typedoc-sidebar.cjs';
import typedocSidebarBcs from '../developer/ts-sdk/bcs/api/typedoc-sidebar.cjs';
import typedocSidebarIscSdk from '../developer/ts-sdk/isc-sdk/api/typedoc-sidebar.cjs';

const tsSDK = [
    {
        type: 'category',
        label: '@iota/iota-sdk',
        items: [
            'developer/ts-sdk/typescript/index', 
            'developer/ts-sdk/typescript/install', 
            'developer/ts-sdk/typescript/hello-iota', 
            'developer/ts-sdk/typescript/faucet', 
            'developer/ts-sdk/typescript/iota-client', 
            'developer/ts-sdk/typescript/graphql', 
            {
                type: 'category',
                label: 'Transaction Building',
                items: [
                    'developer/ts-sdk/typescript/transaction-building/basics', 
                    'developer/ts-sdk/typescript/transaction-building/gas', 
                    'developer/ts-sdk/typescript/transaction-building/sponsored-transactions', 
                    'developer/ts-sdk/typescript/transaction-building/offline', 
                ],
            },
            {
                type: 'category',
                label: 'Cryptography',
                items: [
                    'developer/ts-sdk/typescript/cryptography/keypairs', 
                    'developer/ts-sdk/typescript/cryptography/multisig', 
                ],
            },
            'developer/ts-sdk/typescript/utils', 
            'developer/ts-sdk/typescript/bcs', 
            'developer/ts-sdk/typescript/executors', 
            'developer/ts-sdk/typescript/plugins', 
            {
                type: 'category',
                label: 'Owned Object Pool',
                items: [
                    'developer/ts-sdk/typescript/owned-object-pool/index', 
                    'developer/ts-sdk/typescript/owned-object-pool/overview', 
                    'developer/ts-sdk/typescript/owned-object-pool/local-development', 
                    'developer/ts-sdk/typescript/owned-object-pool/custom-split-strategy', 
                    'developer/ts-sdk/typescript/owned-object-pool/examples', 
                ],
            },
            {
                type: 'category',
                label: 'API Reference',
                items: typedocSidebarIOTASdk,
                link: { type: 'doc', id: 'developer/ts-sdk/typescript/api/index' },
            },
        ],
    },
    {
        type: 'category',
        label: '@iota/dapp-kit',
        items: [
            'developer/ts-sdk/dapp-kit/index', 
            'developer/ts-sdk/dapp-kit/create-dapp', 
            'developer/ts-sdk/dapp-kit/iota-client-provider', 
            'developer/ts-sdk/dapp-kit/rpc-hooks', 
            'developer/ts-sdk/dapp-kit/wallet-provider', 
            {
                type: 'category',
                label: 'Wallet Components',
                items: [
                    'developer/ts-sdk/dapp-kit/wallet-components/ConnectButton', 
                    'developer/ts-sdk/dapp-kit/wallet-components/ConnectModal', 
                ],
            },
            {
                type: 'category',
                label: 'Wallet Hooks',
                items: [
                    'developer/ts-sdk/dapp-kit/wallet-hooks/useWallets', 
                    'developer/ts-sdk/dapp-kit/wallet-hooks/useAccounts', 
                    'developer/ts-sdk/dapp-kit/wallet-hooks/useCurrentWallet', 
                    'developer/ts-sdk/dapp-kit/wallet-hooks/useCurrentAccount', 
                    'developer/ts-sdk/dapp-kit/wallet-hooks/useAutoConnectWallet', 
                    'developer/ts-sdk/dapp-kit/wallet-hooks/useConnectWallet', 
                    'developer/ts-sdk/dapp-kit/wallet-hooks/useDisconnectWallet', 
                    'developer/ts-sdk/dapp-kit/wallet-hooks/useSwitchAccount', 
                    'developer/ts-sdk/dapp-kit/wallet-hooks/useReportTransactionEffects', 
                    'developer/ts-sdk/dapp-kit/wallet-hooks/useSignPersonalMessage', 
                    'developer/ts-sdk/dapp-kit/wallet-hooks/useSignTransaction', 
                    'developer/ts-sdk/dapp-kit/wallet-hooks/useSignAndExecuteTransaction', 
                ],
            },
            'developer/ts-sdk/dapp-kit/themes',
            {
                type: 'category',
                label: 'API Reference',
                items: typedocSidebarDappKit,
                link: { type: 'doc', id: 'developer/ts-sdk/dapp-kit/api/index' },
            },
        ],
    },
    {
        type: 'category',
        label: '@iota/kiosk',
        items: [
            'developer/ts-sdk/kiosk/index', 
            {
                type: 'category',
                label: 'Kiosk Client',
                items: [
                    'developer/ts-sdk/kiosk/kiosk-client/introduction', 
                    'developer/ts-sdk/kiosk/kiosk-client/querying', 
                    {
                        type: 'category',
                        label: 'Kiosk Transactions',
                        items: [
                            'developer/ts-sdk/kiosk/kiosk-client/kiosk-transaction/kiosk-transaction', 
                            'developer/ts-sdk/kiosk/kiosk-client/kiosk-transaction/managing', 
                            'developer/ts-sdk/kiosk/kiosk-client/kiosk-transaction/purchasing', 
                            'developer/ts-sdk/kiosk/kiosk-client/kiosk-transaction/examples', 
                        ],
                    },
                    {
                        type: 'category',
                        label: 'Transfer Policy Transactions',
                        items: [
                            'developer/ts-sdk/kiosk/kiosk-client/transfer-policy-transaction/introduction', 
                            'developer/ts-sdk/kiosk/kiosk-client/transfer-policy-transaction/using-the-manager', 
                        ],
                    },
                ],
            },
            'developer/ts-sdk/kiosk/advanced-examples',
            {
                type: 'category',
                label: 'API Reference',
                items: typedocSidebarKiosk,
                link: { type: 'doc', id: 'developer/ts-sdk/kiosk/api/index' },
            },
        ],
    },
    {
        type: 'category',
        label: '@iota/graphql-transport',
        items: [
            {
                type: 'category',
                label: 'API Reference',
                items: typedocSidebarGraphqlTransport,
                link: { type: 'doc', id: 'developer/ts-sdk/graphql-transport/api/index' },
            },
        ],
    },
    {
        type: 'category',
        label: '@iota/wallet-standard',
        items: [
            {
                type: 'category',
                label: 'API Reference',
                items: typedocSidebarWalletStandard,
                link: { type: 'doc', id: 'developer/ts-sdk/wallet-standard/api/index' },
            },
        ],
    },
    {
        type: 'category',
        label: '@iota/ledgerjs-hw-app-iota',
        items: [
            {
                type: 'category',
                label: 'API Reference',
                items: typedocSidebarLedger,
                link: { type: 'doc', id: 'developer/ts-sdk/ledgerjs-hw-app-iota/api/index' },
            },
        ],
    },
    {
        type: 'category',
        label: '@iota/signers',
        items: [
            {
                type: 'category',
                label: 'API Reference',
                items: typedocSidebarSigners,
                link: { type: 'doc', id: 'developer/ts-sdk/signers/api/index' },
            },
        ],
    },
    {
        type: 'category',
        label: '@iota/bcs',
        items: [
            {
                type: 'category',
                label: 'API Reference',
                items: typedocSidebarBcs,
                link: { type: 'doc', id: 'developer/ts-sdk/bcs/api/index' },
            },
        ],
    },
    {
        type: 'category',
        label: '@iota/isc-sdk',
        items: [
            {
                type: 'category',
                label: 'API Reference',
                items: typedocSidebarIscSdk,
                link: { type: 'doc', id: 'developer/ts-sdk/isc-sdk/api/index' },
            },
        ],
    },
];

module.exports = tsSDK;
