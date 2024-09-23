// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { GrowthBook } from '@growthbook/growthbook';
import { Network, getAppsBackend } from '@iota/iota-sdk/client';
import Browser from 'webextension-polyfill';

function getGrowthbookConfig(env: string) {
    if (env === 'development') {
        return {
            clientKey: 'development',
            enableDevMode: true,
        };
    }
    if (env === 'production') {
        return {
            clientKey: 'production',
            enableDevMode: false,
        };
    }

    // return for local by default
    return {
        clientKey: 'development',
        enableDevMode: true,
    };
}

export const growthbook = new GrowthBook({
    apiHost: getAppsBackend(),
    clientKey: getGrowthbookConfig(process.env.NODE_ENV as string).clientKey,
    enableDevMode: getGrowthbookConfig(process.env.NODE_ENV as string).enableDevMode,
});

/**
 * This is a list of feature keys that are used in wallet
 * https://docs.growthbook.io/app/features#feature-keys
 */
export enum Feature {
    AccountFinder = 'account-finder',
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
        beta: process.env.WALLET_RC || false,
    });
}

// Initialize growthbook to default attributes:
setAttributes();
