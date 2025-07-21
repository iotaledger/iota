// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Controller, Get } from '@nestjs/common';
import { Feature } from '@iota/core/enums/features.enums';
import { Network } from '@iota/iota-sdk/client';

const KNOWN_ADDRESSES = {
    '0x0': 'IOTA System Account',
    '0x1': 'Move stdlib Package',
    '0x2': 'IOTA Framework Package',
    '0x3': 'IOTA System Package',
    '0x5': 'IOTA System Object',
    '0x6': 'IOTA System Clock',
    '0x7': 'IOTA Authenticator Object',
    '0x8': 'IOTA Randonmness Object',
    '0x9': 'Bridge Object',
    '0x107a': 'Stardust Package',
    '0xb': 'Bridge Package',
    '0x403': 'IOTA Denylist Object',
    '0x7b4a34f6a011794f0ecbe5e5beb96102d3eef6122eb929b9f50a8d757bfbdd67': 'IOTA EVM',
};

@Controller('/api/features')
export class FeaturesController {
    @Get('/staging')
    getStagingFeatures() {
        return {
            status: 200,
            features: {
                [Feature.RecognizedPackages]: {
                    defaultValue: [
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
                [Feature.WalletDapps]: {
                    defaultValue: [
                        {
                            name: 'Wallet Dashboard',
                            link: 'https://wallet-dashboard.iota.org/',
                            icon: 'https://iota.org/logo.png',
                            tags: ['Wallet', 'Dashboard'],
                        },
                        {
                            name: 'EVM Bridge',
                            link: 'https://evm-bridge.iota.org/',
                            icon: 'https://iota.org/logo.png',
                            tags: ['EVM', 'Bridge'],
                        },
                    ],
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
                [Feature.KnownAddressAlias]: {
                    defaultValue: {
                        enabled: true,
                        addresses: KNOWN_ADDRESSES,
                    },
                },
                [Feature.KnownIotaEVMCoinTypes]: {
                    defaultValue: [
                        '0x3fbd238eea1f4ce7d797148954518fce853f24a8be01b47388bfa2262602fefa::vusd::VUSD',
                    ],
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
                    defaultValue: [
                        {
                            name: 'Wallet Dashboard',
                            link: 'https://wallet-dashboard.iota.org/',
                            icon: 'https://iota.org/logo.png',
                            tags: ['Wallet', 'Dashboard'],
                        },
                        {
                            name: 'EVM Bridge',
                            link: 'https://evm-bridge.iota.org/',
                            icon: 'https://iota.org/logo.png',
                            tags: ['EVM', 'Bridge'],
                        },
                    ],
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
                [Feature.KnownAddressAlias]: {
                    defaultValue: {
                        enabled: true,
                        addresses: KNOWN_ADDRESSES,
                    },
                },
                [Feature.KnownIotaEVMCoinTypes]: {
                    defaultValue: [
                        '0xd3b63e603a78786facf65ff22e79701f3e824881a12fa3268d62a75530fe904f::vusd::VUSD',
                    ],
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
