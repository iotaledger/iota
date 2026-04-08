// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Controller, Get } from '@nestjs/common';
import { Feature } from '@iota/core/enums/features.enums';
import { Network } from '@iota/iota-sdk/client';
import {
    NAME_ADDRESS_RESOLUTION_FEATURE,
    KNOWN_ADDRESSES_ALIASES,
    RECOGNIZED_PACKAGES,
} from './features.constants';
import { RECOGNIZED_DAPPS } from './dapps.constants';

@Controller('/api/features')
export class FeaturesController {
    @Get('/staging')
    getStagingFeatures() {
        return {
            status: 200,
            features: {
                [Feature.RecognizedPackages]: {
                    defaultValue: RECOGNIZED_PACKAGES,
                },
                [Feature.WalletSentryTracing]: {
                    defaultValue: 0.0025,
                },
                [Feature.WalletDapps]: {
                    defaultValue: RECOGNIZED_DAPPS,
                },
                [Feature.WalletBalanceRefetchInterval]: {
                    defaultValue: 1000,
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
                [Feature.WalletPasskeys]: {
                    defaultValue: {
                        [Network.Mainnet]: true,
                        [Network.Devnet]: true,
                        [Network.Testnet]: true,
                        [Network.Localnet]: true,
                        [Network.Custom]: true,
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
                        [Network.Devnet]: true,
                        [Network.Testnet]: true,
                        [Network.Localnet]: true,
                        [Network.Custom]: true,
                    },
                },
                [Feature.KnownAddressAlias]: {
                    defaultValue: { enabled: true, addresses: KNOWN_ADDRESSES_ALIASES },
                },
                [Feature.KnownIotaEVMCoinTypes]: {
                    defaultValue: [
                        '0x3fbd238eea1f4ce7d797148954518fce853f24a8be01b47388bfa2262602fefa::vusd::VUSD',
                        '0xe1e88f4962b3ea96cfad19aee42f666b04bbce4dc4327c3cd63f1b8ff16e13b2::tool_coin::TOOL_COIN',
                    ],
                },
                [Feature.IotaNames]: {
                    defaultValue: NAME_ADDRESS_RESOLUTION_FEATURE,
                },
                [Feature.ExplorerTFIdentity]: {
                    defaultValue: true,
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
                    defaultValue: RECOGNIZED_PACKAGES,
                },
                [Feature.WalletSentryTracing]: {
                    defaultValue: 0.0025,
                },
                [Feature.WalletDapps]: {
                    defaultValue: RECOGNIZED_DAPPS,
                },
                [Feature.WalletBalanceRefetchInterval]: {
                    defaultValue: 1000,
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
                [Feature.WalletPasskeys]: {
                    defaultValue: {
                        [Network.Mainnet]: true,
                        [Network.Devnet]: true,
                        [Network.Testnet]: true,
                        [Network.Localnet]: true,
                        [Network.Custom]: true,
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
                    defaultValue: { enabled: true, addresses: KNOWN_ADDRESSES_ALIASES },
                },
                [Feature.KnownIotaEVMCoinTypes]: {
                    defaultValue: [
                        '0xd3b63e603a78786facf65ff22e79701f3e824881a12fa3268d62a75530fe904f::vusd::VUSD',
                    ],
                },
                [Feature.IotaNames]: {
                    defaultValue: NAME_ADDRESS_RESOLUTION_FEATURE,
                },
                [Feature.ExplorerTFIdentity]: {
                    defaultValue: true,
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
