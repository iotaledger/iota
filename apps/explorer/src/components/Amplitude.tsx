// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useRef } from 'react';
import { useLocation } from 'react-router-dom';
import { type Network } from '@iota/iota-sdk/client';
import { setNetworkGroup } from '@iota/core';
import { ampli, initAmplitude } from '~/lib/utils';
import { useNetwork } from '~/hooks';

export function Amplitude() {
    const [network] = useNetwork();
    const location = useLocation();
    const previousUrlRef = useRef<string>('');

    // Initialize Amplitude once with the current network
    useEffect(() => {
        initAmplitude(network as Network);
    }, []);

    // Track network changes
    useEffect(() => {
        if (!ampli.client) return;

        setNetworkGroup(ampli.client, network as Network);
    }, [network]);

    // Track page changes
    useEffect(() => {
        if (!ampli.isLoaded) return;

        const search = location.search || '';
        const hash = location.hash || '';
        const currentUrl = `${location.pathname}${search}${hash}`;

        // Only call openedIotaExplorer if the URL has actually changed
        if (currentUrl !== previousUrlRef.current) {
            previousUrlRef.current = currentUrl;
            ampli.openedIotaExplorer({
                pageDomain: window.location.hostname,
                pagePath: location.pathname,
                pageUrl: window.location.href,
                activeNetwork: network,
            });
        }
    }, [location.pathname, location.search, location.hash, network]);

    return null;
}
