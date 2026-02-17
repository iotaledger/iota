// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import * as amplitude from '@amplitude/analytics-browser';
import { LogLevel, type UserSession } from '@amplitude/analytics-types';
import { attachEnvironmentPlugin, getAmplitudeConsentStatus, PersistableStorage } from '@iota/core';

import { ampli } from './ampli';
import { getDefaultNetwork } from '../../config';

const IS_ENABLED =
    import.meta.env.VITE_BUILD_ENV === 'production' &&
    import.meta.env.VITE_AMPLITUDE_ENABLED === 'true';

const IS_DEV = import.meta.env.VITE_BUILD_ENV !== 'production';

export const persistableStorage = new PersistableStorage<UserSession>();

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
                    attribution: IS_ENABLED,
                    fileDownloads: IS_ENABLED,
                    formInteractions: IS_ENABLED,
                    pageViews: IS_ENABLED,
                    sessions: IS_ENABLED,
                },
                // set LogLevel to Debug for more verbose logging during development
                logLevel: LogLevel.None,
            },
        },
    }).promise;

    setNetworkGroup(getDefaultNetwork());

    window.addEventListener('pagehide', () => {
        amplitude.setTransport('beacon');
        amplitude.flush();
    });

    // Add environment plugin to set prefix dev events
    ampli.client.add(attachEnvironmentPlugin(IS_DEV));
}

export function getUrlWithDeviceId(url: URL) {
    const deviceId = ampli.client.getDeviceId();
    if (deviceId) {
        url.searchParams.set('amplitude_device_id', deviceId);
    }
    return url;
}

function setNetworkGroup(network: string): void {
    ampli.client.setGroup('activeNetwork', network);
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
