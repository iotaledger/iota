// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Injectable } from '@nestjs/common';
import { Feature } from '@iota/core/enums/features.enums';
import { Network } from '@iota/iota-sdk/client';
import {
    NAME_ADDRESS_RESOLUTION_FEATURE,
    KNOWN_ADDRESSES_ALIASES,
    RECOGNIZED_PACKAGES,
} from './features.constants';
import { VERSIONED_FEATURES } from './versioned-features';
import { isValidSemver, semverGte } from './version.utils';

type FeaturesMap = Record<string, { defaultValue: unknown }>;
type Environment = 'staging' | 'production';

@Injectable()
export class FeaturesService {
    getStagingFeatures(version?: string) {
        const features = this.buildStagingFeatures();
        const filtered = this.applyVersionFilter(features, 'staging', version);

        return {
            status: 200,
            features: filtered,
            dateUpdated: new Date().toISOString(),
        };
    }

    getProductionFeatures(version?: string) {
        const features = this.buildProductionFeatures();
        const filtered = this.applyVersionFilter(features, 'production', version);

        return {
            status: 200,
            features: filtered,
            dateUpdated: new Date().toISOString(),
        };
    }

    /**
     * Apply version-based filtering to the features map.
     *
     * - If no version is provided (or the version is invalid), all features are returned as-is
     *   for backward compatibility.
     * - If a version is provided and a feature has a rule in VERSIONED_FEATURES:
     *   - version < minVersion → the feature is excluded from the response.
     *   - version >= minVersion → the feature is included. If an environment-specific override
     *     value exists, it replaces the hardcoded defaultValue.
     * - Features not listed in VERSIONED_FEATURES are always included.
     */
    applyVersionFilter(
        features: FeaturesMap,
        environment: Environment,
        version?: string,
    ): FeaturesMap {
        if (!version || !isValidSemver(version)) {
            return features;
        }

        const result: FeaturesMap = {};

        for (const [key, value] of Object.entries(features)) {
            const rule = VERSIONED_FEATURES[key as Feature];

            if (!rule) {
                // No version constraint — always include.
                result[key] = value;
                continue;
            }

            if (!semverGte(version, rule.minVersion)) {
                // Client version is below the minimum — exclude this feature.
                continue;
            }

            // Client version satisfies the constraint — include with optional override.
            const override = rule[environment];
            if (override !== undefined) {
                result[key] = { defaultValue: override };
            } else {
                result[key] = value;
            }
        }

        return result;
    }

    private buildStagingFeatures(): FeaturesMap {
        return {
            [Feature.RecognizedPackages]: {
                defaultValue: RECOGNIZED_PACKAGES,
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
                defaultValue: false,
            },
        };
    }

    private buildProductionFeatures(): FeaturesMap {
        return {
            [Feature.RecognizedPackages]: {
                defaultValue: RECOGNIZED_PACKAGES,
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
                    [Network.Mainnet]: false,
                    [Network.Devnet]: false,
                    [Network.Testnet]: false,
                    [Network.Localnet]: false,
                    [Network.Custom]: false,
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
                defaultValue: false,
            },
        };
    }
}
