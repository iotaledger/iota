// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import * as amplitude from '@amplitude/analytics-browser';
import { LogLevel, type UserSession } from '@amplitude/analytics-types';
import { getAmplitudeConsentStatus, PersistableStorage } from '@iota/core';

import { ampli } from './ampli';
import { getDefaultNetwork, CONFIG } from '../../config';

const IS_ENABLED = import.meta.env.VITE_BUILD_ENV === 'production';

export const persistableStorage = new PersistableStorage<UserSession>();

const GROUP_KEY = 'network';

export enum BridgeDirection {
    L1ToL2 = 'l1_to_l2',
    L2ToL1 = 'l2_to_l1',
}

export enum Layer {
    L1 = 'l1',
    L2 = 'l2',
}

const ApiKey = {
    production: 'bc860617cd112db8797d4b8809b15142',
};

export async function initAmplitude() {
    // Check consent status to determine initial opt-out state
    const consentStatus = getAmplitudeConsentStatus();

    if (ampli.isLoaded || consentStatus === 'declined') {
        return;
    }

    await ampli.load({
        disabled: !IS_ENABLED,
        client: {
            apiKey: ApiKey.production,
            configuration: {
                optOut: false,
                autocapture: {
                    pageViews: IS_ENABLED,
                    sessions: IS_ENABLED,
                },
                logLevel: IS_ENABLED ? LogLevel.Warn : LogLevel.None,
            },
        },
    }).promise;

    setNetworkGroup();

    window.addEventListener('pagehide', () => {
        amplitude.setTransport('beacon');
        amplitude.flush();
    });
}

export function getUrlWithDeviceId(url: URL) {
    const deviceId = ampli.client.getDeviceId();
    if (deviceId) {
        url.searchParams.set('amplitude_device_id', deviceId);
    }
    return url;
}

/**
 * Update the user's network group in Amplitude.
 * This allows filtering events by network in Amplitude analytics.
 */
function setNetworkGroup(): void {
    if (!ampli.client) {
        console.warn('Amplitude client is not initialized. Cannot set network group.');
        return;
    }

    // Get the L1 network name from the current configuration
    const networkName = CONFIG.L1.networkName || getDefaultNetwork();

    ampli.client.setGroup(GROUP_KEY, networkName);
}

/**
 * Set wallet information as groups for connected wallets.
 * Groups are attached to all future events for better segmentation and cohort analysis.
 */
export function setWalletUserGroup(walletInfo: {
    l1WalletType?: string;
    l2WalletType?: string;
    l2ChainId?: string;
}) {
    if (!ampli.client) return;

    if (walletInfo.l1WalletType) {
        ampli.client.setGroup('l1_wallet_type', walletInfo.l1WalletType);
    }

    if (walletInfo.l2WalletType) {
        ampli.client.setGroup('l2_wallet_type', walletInfo.l2WalletType);
    }

    if (walletInfo.l2ChainId) {
        ampli.client.setGroup('l2_chain_id', walletInfo.l2ChainId);
    }
}

/**
 * Clear wallet groups when disconnected.
 */
export function clearWalletUserGroup(layer: 'l1' | 'l2') {
    if (!ampli.client) return;

    if (layer === 'l1') {
        ampli.client.setGroup('l1_wallet_type', []);
    } else {
        ampli.client.setGroup('l2_wallet_type', []);
        ampli.client.setGroup('l2_chain_id', []);
    }
}
