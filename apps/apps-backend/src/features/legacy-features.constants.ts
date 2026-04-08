// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// This file contains feature flags that have been removed from the consumer
// apps but are kept here, permanently enabled, for backwards compatibility
// with older clients that may still be reading them from the apps-backend.

import { Network } from '@iota/iota-sdk/client';
import { NAME_ADDRESS_RESOLUTION_FEATURE } from './features.constants';

export const LEGACY_FEATURE_FLAGS = {
    'wallet-passkey-accounts': {
        defaultValue: {
            [Network.Mainnet]: true,
            [Network.Devnet]: true,
            [Network.Testnet]: true,
            [Network.Localnet]: true,
            [Network.Custom]: true,
        },
    },
    'account-finder': {
        defaultValue: true,
    },
    migration: {
        defaultValue: true,
    },
    'supply-increase-vesting': {
        defaultValue: true,
    },
    'iota-names': {
        defaultValue: NAME_ADDRESS_RESOLUTION_FEATURE,
    },
};
