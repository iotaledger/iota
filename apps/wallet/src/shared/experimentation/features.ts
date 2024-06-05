// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0
import { GrowthBook } from '@growthbook/growthbook';
import { Network, getAppsBackend } from '@iota/iota.js/client';
import Browser from 'webextension-polyfill';

// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
export const growthbook = new GrowthBook({
    apiHost: getAppsBackend(),
    clientKey: process.env.NODE_ENV === 'development' ? 'development' : 'production',
    enableDevMode: process.env.NODE_ENV === 'development',
});

/**
 * This is a list of feature keys that are used in wallet
 * https://docs.growthbook.io/app/features#feature-keys
 */
export enum FEATURES {
    USE_LOCAL_TXN_SERIALIZER = 'use-local-txn-serializer',
    WALLET_DAPPS = 'wallet-dapps',
    WALLET_BALANCE_REFETCH_INTERVAL = 'wallet-balance-refetch-interval',
    WALLET_ACTIVITY_REFETCH_INTERVAL = 'wallet-activity-refetch-interval',
    WALLET_EFFECTS_ONLY_SHARED_TRANSACTION = 'wallet-effects-only-shared-transaction',
    WALLET_APPS_BANNER_CONFIG = 'wallet-apps-banner-config',
    WALLET_INTERSTITIAL_CONFIG = 'wallet-interstitial-config',
    WALLET_DEFI = 'wallet-defi',
    WALLET_FEE_ADDRESS = 'wallet-fee-address',
    DEEP_BOOK_CONFIGS = 'deep-book-configs',
}

export function setAttributes(network?: { network: Network; customRpc?: string | null }) {
    const activeNetwork = network
        ? network.network === Network.Custom && network.customRpc
            ? network.customRpc
            : network.network.toUpperCase()
        : null;

    growthbook.setAttributes({
        network: activeNetwork,
        version: Browser.runtime.getManifest().version,
        beta: process.env.WALLET_BETA || false,
    });
}

// Initialize growthbook to default attributes:
setAttributes();
