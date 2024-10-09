// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Controller, Get, Query } from '@nestjs/common';
import { Feature } from '@iota/core';

@Controller('/api/features')
export class FeaturesController {
    @Get('/development')
    getDevelopmentFeatures() {
        return {
            status: 200,
            features: {
                [Feature.RecognizedPackages]: {
                    defaultValue: [],
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
                [Feature.KioskOriginbytePackageid]: {
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
                    defaultValue: false,
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
                    defaultValue: [],
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
                [Feature.KioskOriginbytePackageid]: {
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
                    defaultValue: false,
                },
            },
            dateUpdated: new Date().toISOString(),
        };
    }

    @Get('/apps')
    getAppsFeatures(@Query('network') network: string) {
        return {
            status: 200,
            apps: [], // Note: we'll add wallet dapps when evm will be ready
            dateUpdated: new Date().toISOString(),
        };
    }
}
