// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/**
 * This is a list of feature keys that are used in wallet
 * https://docs.growthbook.io/app/features#feature-keys
 */
export enum Feature {
    AccountFinder = 'account-finder',
    WalletDapps = 'wallet-dapps',
    WalletBalanceRefetchInterval = 'wallet-balance-refetch-interval',
    WalletAppsBannerConfig = 'wallet-apps-banner-config',
    WalletInterstitialConfig = 'wallet-interstitial-config',
    RecognizedPackages = 'recognized-packages',
    WalletSentryTracing = 'wallet-sentry-tracing',
    KioskOriginbytePackageid = 'kiosk-originbyte-packageid',
    PollingTxnTable = 'polling-txn-table',
    NetworkOutageOverride = 'network-outage-override',
    ModuleSourceVerification = 'module-source-verification',
}
