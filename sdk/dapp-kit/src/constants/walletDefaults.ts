// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { IotaWalletFeatures, WalletWithRequiredFeatures } from '@iota/wallet-standard';
import { STASHED_WALLET_NAME } from '@iota/zksend';

import { createInMemoryStore } from '../utils/stateStorage.js';

export const IOTA_WALLET_NAME = 'Iota Wallet';

export const DEFAULT_STORAGE =
    typeof window !== 'undefined' && window.localStorage ? localStorage : createInMemoryStore();

export const DEFAULT_STORAGE_KEY = 'iota-dapp-kit:wallet-connection-info';

const SIGN_FEATURES = [
    'iota:signTransaction',
    'iota:signTransactionBlock',
] satisfies (keyof IotaWalletFeatures)[];

export const DEFAULT_WALLET_FILTER = (wallet: WalletWithRequiredFeatures) =>
    SIGN_FEATURES.some((feature) => wallet.features[feature]);

export const DEFAULT_PREFERRED_WALLETS = [IOTA_WALLET_NAME, STASHED_WALLET_NAME];
