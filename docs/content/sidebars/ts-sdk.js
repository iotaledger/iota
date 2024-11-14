// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import typedocSidebar from '../references/ts-sdk/api/typedoc-sidebar.cjs';
const references = [
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
    {
        type: 'category',
        label: 'API',
        link: {
            type: 'doc',
            id: 'references/ts-sdk/api/index',
        },
        items: typedocSidebar,
    },
];

module.exports = references;
