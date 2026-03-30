// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { AppsBackendClient } from '@iota/apps-backend-client';
import { getAppsBackend } from '@iota/core';
import { Network } from '@iota/iota-sdk/client';
import Browser from 'webextension-polyfill';

export const appsBackendClient = new AppsBackendClient({
    url: getAppsBackend(),
});

export function setAttributes(network?: { network: Network; customRpc?: string | null }) {
    const activeNetwork = network
        ? network.network === Network.Custom && network.customRpc
            ? network.customRpc
            : network.network.toUpperCase()
        : null;

    appsBackendClient.setAttributes({
        network: activeNetwork,
        version: Browser.runtime.getManifest().version,
        rc: process.env.IS_RC || false,
    });
}

// Initialize to default attributes:
setAttributes();
