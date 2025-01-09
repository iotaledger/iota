// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { IotaWalletFeatures, WalletWithRequiredFeatures } from '@iota/wallet-standard';

import { createInMemoryStore } from '../utils/stateStorage.js';

export const IOTA_WALLET_NAME = 'IOTA Wallet';

export const DEFAULT_STORAGE =
    typeof window !== 'undefined' && window.localStorage ? localStorage : createInMemoryStore();

export const DEFAULT_STORAGE_KEY = 'iota-dapp-kit:wallet-connection-info';

const SIGN_FEATURES = ['iota:signTransaction'] satisfies (keyof IotaWalletFeatures)[];

export const DEFAULT_WALLET_FILTER = (wallet: WalletWithRequiredFeatures) =>
    SIGN_FEATURES.some((feature) => wallet.features[feature]);

export const DEFAULT_PREFERRED_WALLETS = [IOTA_WALLET_NAME];

const WALLET_CHROME_EXTENSION_ID = 'iidjkmdceolghepehaaddojmnjnkkija';

export const WALLET_DOWNLOAD_URL =
    'https://chromewebstore.google.com/detail/' + WALLET_CHROME_EXTENSION_ID;
