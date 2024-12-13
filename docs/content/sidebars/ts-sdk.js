// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import typedocSidebar from '../ts-sdk/api/typedoc-sidebar.cjs';
const tsSDK = [
    {
        type: 'category',
        label: 'Typescript SDK',
        items: [
            'ts-sdk/typescript/index',
            'ts-sdk/typescript/install',
            'ts-sdk/typescript/hello-iota',
            'ts-sdk/typescript/faucet',
            'ts-sdk/typescript/iota-client',
            'ts-sdk/typescript/graphql',
            {
                type: 'category',
                label: 'Transaction Building',
                items: [
                    'ts-sdk/typescript/transaction-building/basics',
                    'ts-sdk/typescript/transaction-building/gas',
                    'ts-sdk/typescript/transaction-building/sponsored-transactions',
                    'ts-sdk/typescript/transaction-building/offline',
                ],
            },
            {
                type: 'category',
                label: 'Cryptography',
                items: [
                    'ts-sdk/typescript/cryptography/keypairs',
                    'ts-sdk/typescript/cryptography/multisig',
                ],
            },
            'ts-sdk/typescript/utils',
            'ts-sdk/typescript/bcs',
            'ts-sdk/typescript/executors',
            'ts-sdk/typescript/plugins',
            {
                type: 'category',
                label: 'Owned Object Pool',
                items: [
                    'ts-sdk/typescript/owned-object-pool/index',
                    'ts-sdk/typescript/owned-object-pool/overview',
                    'ts-sdk/typescript/owned-object-pool/local-development',
                    'ts-sdk/typescript/owned-object-pool/custom-split-strategy',
                    'ts-sdk/typescript/owned-object-pool/examples',
                ],
            },
        ],
    },
    {
        type: 'category',
        label: 'dApp Kit',
        items: [
            'ts-sdk/dapp-kit/index',
            'ts-sdk/dapp-kit/create-dapp',
            'ts-sdk/dapp-kit/iota-client-provider',
            'ts-sdk/dapp-kit/rpc-hooks',
            'ts-sdk/dapp-kit/wallet-provider',
            {
                type: 'category',
                label: 'Wallet Components',
                items: [
                    'ts-sdk/dapp-kit/wallet-components/ConnectButton',
                    'ts-sdk/dapp-kit/wallet-components/ConnectModal',
                ],
            },
            {
                type: 'category',
                label: 'Wallet Hooks',
                items: [
                    'ts-sdk/dapp-kit/wallet-hooks/useWallets',
                    'ts-sdk/dapp-kit/wallet-hooks/useAccounts',
                    'ts-sdk/dapp-kit/wallet-hooks/useCurrentWallet',
                    'ts-sdk/dapp-kit/wallet-hooks/useCurrentAccount',
                    'ts-sdk/dapp-kit/wallet-hooks/useAutoConnectWallet',
                    'ts-sdk/dapp-kit/wallet-hooks/useConnectWallet',
                    'ts-sdk/dapp-kit/wallet-hooks/useDisconnectWallet',
                    'ts-sdk/dapp-kit/wallet-hooks/useSwitchAccount',
                    'ts-sdk/dapp-kit/wallet-hooks/useReportTransactionEffects',
                    'ts-sdk/dapp-kit/wallet-hooks/useSignPersonalMessage',
                    'ts-sdk/dapp-kit/wallet-hooks/useSignTransaction',
                    'ts-sdk/dapp-kit/wallet-hooks/useSignAndExecuteTransaction',
                ],
            },
            'ts-sdk/dapp-kit/themes',
        ],
    },
    {
        type: 'category',
        label: 'Kiosk SDK',
        items: [
            'ts-sdk/kiosk/index',
            {
                type: 'category',
                label: 'Kiosk Client',
                items: [
                    'ts-sdk/kiosk/kiosk-client/introduction',
                    'ts-sdk/kiosk/kiosk-client/querying',
                    {
                        type: 'category',
                        label: 'Kiosk Transactions',
                        items: [
                            'ts-sdk/kiosk/kiosk-client/kiosk-transaction/kiosk-transaction',
                            'ts-sdk/kiosk/kiosk-client/kiosk-transaction/managing',
                            'ts-sdk/kiosk/kiosk-client/kiosk-transaction/purchasing',
                            'ts-sdk/kiosk/kiosk-client/kiosk-transaction/examples',
                        ],
                    },
                    {
                        type: 'category',
                        label: 'Transfer Policy Transactions',
                        items: [
                            'ts-sdk/kiosk/kiosk-client/transfer-policy-transaction/introduction',
                            'ts-sdk/kiosk/kiosk-client/transfer-policy-transaction/using-the-manager',
                        ],
                    },
                ],
            },
            'ts-sdk/kiosk/advanced-examples',
        ],
    },
    'ts-sdk/bcs',
    {
        type: 'category',
        label: 'API',
        link: {
            type: 'doc',
            id: 'ts-sdk/api/index',
        },
        items: typedocSidebar,
    },
];

module.exports = tsSDK;
