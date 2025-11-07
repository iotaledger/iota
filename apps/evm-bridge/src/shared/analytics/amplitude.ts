// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import * as amplitude from '@amplitude/analytics-browser';
import { LogLevel, TransportType, type UserSession } from '@amplitude/analytics-types';
import { PersistableStorage } from '@iota/core';

import { ampli } from './ampli';

const IS_PROD_ENV = import.meta.env.VITE_BUILD_ENV === 'production';

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
    await ampli.load({
        disabled: !IS_PROD_ENV,
        client: {
            apiKey: ApiKey.production,
            configuration: {
                logLevel: IS_PROD_ENV ? LogLevel.Warn : amplitude.Types.LogLevel.Debug,
            },
        },
    });

    window.addEventListener('pagehide', () => {
        amplitude.setTransport(TransportType.SendBeacon);
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
