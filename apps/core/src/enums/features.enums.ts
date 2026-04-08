// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/**
 * This is a list of feature keys served by the apps-backend.
 */
export enum Feature {
    WalletDapps = 'wallet-dapps',
    WalletBalanceRefetchInterval = 'wallet-balance-refetch-interval',
    WalletAppsBannerConfig = 'wallet-apps-banner-config',
    WalletInterstitialConfig = 'wallet-interstitial-config',
    RecognizedPackages = 'recognized-packages',
    WalletSentryTracing = 'wallet-sentry-tracing',
    PollingTxnTable = 'polling-txn-table',
    NetworkOutageOverride = 'network-outage-override',
    ModuleSourceVerification = 'module-source-verification',
    WalletEffectsOnlySharedTransaction = 'wallet-effects-only-shared-transaction',
    FiatConversion = 'fiat-conversion',
    KnownAddressAlias = 'known-address-alias',
    KnownIotaEVMCoinTypes = 'known-iota-evm-coin-types',
    ValidatorAddressAlias = 'validator-address-alias',
    ExplorerTFIdentity = 'explorer-trust-framework-identity',
}
