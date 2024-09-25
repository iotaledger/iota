// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { WalletWithRequiredFeatures } from '@iota/wallet-standard';

import { createInMemoryStore } from '../utils/stateStorage.js';

export const IOTA_WALLET_NAME = 'Iota Wallet';

export const DEFAULT_STORAGE =
    typeof window !== 'undefined' && window.localStorage ? localStorage : createInMemoryStore();

export const DEFAULT_STORAGE_KEY = 'iota-dapp-kit:wallet-connection-info';

export const DEFAULT_REQUIRED_FEATURES: (keyof WalletWithRequiredFeatures['features'])[] = [
    'iota:signTransactionBlock',
];

export const DEFAULT_PREFERRED_WALLETS = [IOTA_WALLET_NAME];

const WALLET_CHROME_EXTENSION_ID = 'TODO';

export const WALLET_DOWNLOAD_URL =
    'https://chromewebstore.google.com/detail/' + WALLET_CHROME_EXTENSION_ID;
