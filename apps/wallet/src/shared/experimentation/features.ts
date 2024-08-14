// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { GrowthBook } from '@growthbook/growthbook';
import { Network, getAppsBackend } from '@iota/iota-sdk/client';
import Browser from 'webextension-polyfill';

export const growthbook = new GrowthBook({
    apiHost: getAppsBackend(),
    clientKey: process.env.NODE_ENV === 'development' ? 'development' : 'production',
    enableDevMode: process.env.NODE_ENV === 'development',
});

/**
 * This is a list of feature keys that are used in wallet
 * https://docs.growthbook.io/app/features#feature-keys
 */
export enum Feature {
    UseLocalTxnSerializer = 'use-local-txn-serializer',
    WalletDapps = 'wallet-dapps',
    WalletBalanceRefetchInterval = 'wallet-balance-refetch-interval',
    WalletActivityRefetchInterval = 'wallet-activity-refetch-interval',
    WalletEffectsOnlySharedTransaction = 'wallet-effects-only-shared-transaction',
    WalletAppsBannerConfig = 'wallet-apps-banner-config',
    WalletInterstitialConfig = 'wallet-interstitial-config',
    WalletDefi = 'wallet-defi',
    WalletFeeAddress = 'wallet-fee-address',
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
