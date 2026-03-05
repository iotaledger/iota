// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { LogLevel, type UserSession } from '@amplitude/analytics-types';
import { attachEnvironmentPlugin, getAmplitudeConsentStatus, PersistableStorage } from '@iota/core';

import { ampli } from './ampli';
import { getDefaultNetwork } from '../../config';
import { Identify } from '@amplitude/analytics-browser';

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
                    attribution: false,
                    fileDownloads: false,
                    formInteractions: false,
                    pageViews: IS_ENABLED,
                    sessions: IS_ENABLED,
                    elementInteractions: IS_ENABLED,
                    frustrationInteractions: false,
                    networkTracking: false,
                    webVitals: false,
                    pageUrlEnrichment: IS_ENABLED,
                },
                // set LogLevel to Debug for more verbose logging during development
                logLevel: LogLevel.None,
            },
        },
    }).promise;

    setAmplitudeIdentity();

    window.addEventListener('pagehide', () => {
        ampli.client.setTransport('beacon');
        ampli.flush();
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

type AmplitudeIdentityOptions = {
    activeNetwork?: string;
    l1WalletType?: string;
    l2WalletType?: string;
    l2ChainId?: string;
};

// Track the current identity state to preserve properties across updates
const currentIdentity: Required<AmplitudeIdentityOptions> = {
    activeNetwork: '',
    l1WalletType: '',
    l2WalletType: '',
    l2ChainId: '',
};

export function setAmplitudeIdentity(options?: AmplitudeIdentityOptions): void {
    if (!ampli.isLoaded) {
        return;
    }

    // Update current state with provided options
    if (options?.activeNetwork !== undefined) {
        currentIdentity.activeNetwork = options.activeNetwork;
    } else if (!currentIdentity.activeNetwork) {
        // Initialize activeNetwork on first call
        currentIdentity.activeNetwork = getDefaultNetwork();
    }

    if (options?.l1WalletType !== undefined) {
        currentIdentity.l1WalletType = options.l1WalletType;
    }

    if (options?.l2WalletType !== undefined) {
        currentIdentity.l2WalletType = options.l2WalletType;
    }

    if (options?.l2ChainId !== undefined) {
        currentIdentity.l2ChainId = options.l2ChainId;
    }

    // Build identify event with current state
    const identifyEvent = new Identify();

    // Always set activeNetwork to maintain state
    identifyEvent.set('activeNetwork', currentIdentity.activeNetwork);

    // Set or unset wallet properties based on current state
    if (currentIdentity.l1WalletType) {
        identifyEvent.set('l1WalletType', currentIdentity.l1WalletType);
    } else {
        identifyEvent.unset('l1WalletType');
    }

    if (currentIdentity.l2WalletType) {
        identifyEvent.set('l2WalletType', currentIdentity.l2WalletType);
    } else {
        identifyEvent.unset('l2WalletType');
    }

    if (currentIdentity.l2ChainId) {
        identifyEvent.set('l2ChainId', currentIdentity.l2ChainId);
    } else {
        identifyEvent.unset('l2ChainId');
    }

    ampli.client.identify(identifyEvent);
}
