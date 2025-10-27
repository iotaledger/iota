// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import typedocSidebarTestnet from '../developer/ts-sdk/api/typedoc-sidebar.cjs';
import typedocSidebarDevnet from '../developer/ts-sdk/api/devnet/typedoc-sidebar.cjs';

const tsSDK = [
    {
        type: 'category',
        label: 'Typescript SDK',
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
        ],
    },
    {
        type: 'category',
        label: 'dApp Kit',
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
        ],
    },
    {
        type: 'category',
        label: 'Kiosk SDK',
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
        ],
    },
    'developer/ts-sdk/bcs', 
    {
        type: 'category',
        label: 'API',
        items: [
            {
                type: 'category',
                label: 'Testnet',
                items: typedocSidebarTestnet,
                link: { type: 'doc', id: 'developer/ts-sdk/api/index' }, 
            },
            {
                type: 'category',
                label: 'Devnet',
                items: typedocSidebarDevnet,
                link: { type: 'doc', id: 'developer/ts-sdk/api/devnet/index' }, 
            },
        ],
    },
];

module.exports = tsSDK;
