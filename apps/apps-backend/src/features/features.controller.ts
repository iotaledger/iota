// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Controller, Get } from '@nestjs/common';
import { Feature } from '@iota/core/enums/features.enums';
import { Network } from '@iota/iota-sdk/client';

@Controller('/api/features')
export class FeaturesController {
    @Get('/staging')
    getStagingFeatures() {
        return {
            status: 200,
            features: {
                [Feature.RecognizedPackages]: {
                    defaultValue: [
                        '0xb',
                        '0x2',
                        '0x3',
                        '0x1',
                        '0x107a',
                        '0x0000000000000000000000000000000000000000000000000000000000000002',
                        '0x0000000000000000000000000000000000000000000000000000000000000003',
                        '0x0000000000000000000000000000000000000000000000000000000000000001',
                        '0x000000000000000000000000000000000000000000000000000000000000107a',
                    ],
                },
                [Feature.WalletSentryTracing]: {
                    defaultValue: 0.0025,
                },
                // Note: we'll add wallet dapps when evm will be ready
                [Feature.WalletDapps]: {
                    defaultValue: [],
                },
                [Feature.WalletBalanceRefetchInterval]: {
                    defaultValue: 1000,
                },
                [Feature.KioskOriginbytePackageId]: {
                    defaultValue: '',
                },
                [Feature.WalletAppsBannerConfig]: {
                    defaultValue: {
                        enabled: false,
                        bannerUrl: '',
                        imageUrl: '',
                    },
                },
                [Feature.WalletInterstitialConfig]: {
                    defaultValue: {
                        enabled: false,
                        dismissKey: '',
                        imageUrl: '',
                        bannerUrl: '',
                    },
                },
                [Feature.PollingTxnTable]: {
                    defaultValue: true,
                },
                [Feature.NetworkOutageOverride]: {
                    defaultValue: false,
                },
                [Feature.ModuleSourceVerification]: {
                    defaultValue: true,
                },
                [Feature.AccountFinder]: {
                    defaultValue: true,
                },
                [Feature.StardustMigration]: {
                    defaultValue: true,
                },
                [Feature.SupplyIncreaseVesting]: {
                    defaultValue: true,
                },
                [Feature.FiatConversion]: {
                    defaultValue: {
                        [Network.Mainnet]: true,
                        [Network.Devnet]: false,
                        [Network.Testnet]: false,
                        [Network.Localnet]: false,
                        [Network.Custom]: false,
                    },
                },
            },
            dateUpdated: new Date().toISOString(),
        };
    }

    @Get('/production')
    getProductionFeatures() {
        return {
            status: 200,
            features: {
                [Feature.RecognizedPackages]: {
                    defaultValue: [
                        '0xb',
                        '0x2',
                        '0x3',
                        '0x1',
                        '0x107a',
                        '0x0000000000000000000000000000000000000000000000000000000000000002',
                        '0x0000000000000000000000000000000000000000000000000000000000000003',
                        '0x0000000000000000000000000000000000000000000000000000000000000001',
                        '0x000000000000000000000000000000000000000000000000000000000000107a',
                    ],
                },
                [Feature.WalletSentryTracing]: {
                    defaultValue: 0.0025,
                },
                // Note: we'll add wallet dapps when evm will be ready
                [Feature.WalletDapps]: {
                    defaultValue: [],
                },
                [Feature.WalletBalanceRefetchInterval]: {
                    defaultValue: 1000,
                },
                [Feature.KioskOriginbytePackageId]: {
                    defaultValue: '',
                },
                [Feature.WalletAppsBannerConfig]: {
                    defaultValue: {
                        enabled: false,
                        bannerUrl: '',
                        imageUrl: '',
                    },
                },
                [Feature.WalletInterstitialConfig]: {
                    defaultValue: {
                        enabled: false,
                        dismissKey: '',
                        imageUrl: '',
                        bannerUrl: '',
                    },
                },
                [Feature.PollingTxnTable]: {
                    defaultValue: true,
                },
                [Feature.NetworkOutageOverride]: {
                    defaultValue: false,
                },
                [Feature.ModuleSourceVerification]: {
                    defaultValue: true,
                },
                [Feature.AccountFinder]: {
                    defaultValue: true,
                },
                [Feature.StardustMigration]: {
                    defaultValue: true,
                },
                [Feature.SupplyIncreaseVesting]: {
                    defaultValue: true,
                },
                [Feature.FiatConversion]: {
                    defaultValue: {
                        [Network.Mainnet]: true,
                        [Network.Devnet]: false,
                        [Network.Testnet]: false,
                        [Network.Localnet]: false,
                        [Network.Custom]: false,
                    },
                },
            },
            dateUpdated: new Date().toISOString(),
        };
    }

    @Get('/apps')
    getAppsFeatures() {
        return {
            status: 200,
            apps: [], // Note: we'll add wallet dapps when evm will be ready
            dateUpdated: new Date().toISOString(),
        };
    }
}
