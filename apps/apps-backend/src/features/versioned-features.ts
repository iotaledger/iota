// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Feature } from '@iota/core/enums/features.enums';
import { Network } from '@iota/iota-sdk/client';

/**
 * Defines a version-gated feature rule.
 *
 * - `minVersion`: The minimum semver client version required for this feature to be included.
 * - `staging` (optional): Override value to use in the staging environment when the version
 *   requirement is met. If not provided, the feature's existing hardcoded default is kept.
 * - `production` (optional): Override value to use in the production environment when the
 *   version requirement is met. If not provided, the feature's existing hardcoded default is kept.
 */
export interface VersionedFeatureRule {
    minVersion: string;
    staging?: unknown;
    production?: unknown;
}

/**
 * Declarative map of features that require a minimum client version.
 *
 * How it works:
 * - If a client does NOT send a `?version` query param, all features are returned as-is
 *   (full backward compatibility).
 * - If a client sends `?version=X.Y.Z` and a feature is listed here:
 *   - If version < minVersion → the feature is **excluded** from the response.
 *   - If version >= minVersion → the feature is **included**. If an environment-specific
 *     override value is provided, it replaces the hardcoded defaultValue.
 * - Features NOT listed here are always included regardless of the client version.
 *
 * Example usage:
 * ```
 * [Feature.WalletPasskeys]: {
 *     minVersion: '1.5.0',
 *     production: {
 *         [Network.Mainnet]: true,
 *         [Network.Devnet]: true,
 *         [Network.Testnet]: true,
 *         [Network.Localnet]: true,
 *         [Network.Custom]: true,
 *     },
 * },
 * ```
 */
export const VERSIONED_FEATURES: Partial<Record<Feature, VersionedFeatureRule>> = {
    // Passkeys are enabled in production only for wallet >= 1.5.0.
    // Staging already has all networks enabled, so no staging override is needed.
    [Feature.WalletPasskeys]: {
        minVersion: '1.5.0',
        production: {
            [Network.Mainnet]: true,
            [Network.Devnet]: true,
            [Network.Testnet]: true,
            [Network.Localnet]: true,
            [Network.Custom]: true,
        },
    },
};
