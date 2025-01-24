// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useEffect } from 'react';
import { useLocation } from 'react-router-dom';

import { ampli } from '~/lib/utils';

export function useInitialPageView(activeNetwork: string): void {
    const location = useLocation();

    // Set user properties for the user's page information
    useEffect(() => {
        ampli.identify(undefined);
    }, [location.pathname, activeNetwork]);

    // Log an initial page view event
    useEffect(() => {
        ampli.openedIotaExplorer({
            pageDomain: window.location.hostname,
            pagePath: location.pathname,
            pageUrl: window.location.href,
            activeNetwork,
        });
    }, []);
}
