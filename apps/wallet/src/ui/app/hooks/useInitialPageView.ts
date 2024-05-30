// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ampli } from '_src/shared/analytics/ampli';
import { getCustomNetwork } from '_src/shared/api-env';
import { getNetwork } from '@mysten/sui.js/client';
import { useEffect } from 'react';
import { useLocation } from 'react-router-dom';
import Browser from 'webextension-polyfill';

import { AppType } from '../redux/slices/app/AppType';
import { useActiveAccount } from './useActiveAccount';
import useAppSelector from './useAppSelector';

export function useInitialPageView() {
    const activeAccount = useActiveAccount();
    const location = useLocation();
    const { network, customRpc, activeOrigin, appType } = useAppSelector((state) => state.app);
    const activeNetwork = customRpc ? getCustomNetwork(customRpc).url : getNetwork(network)?.url;
    const isFullScreen = appType === AppType.fullscreen;

    useEffect(() => {
        ampli.identify(undefined, {
            activeNetwork,
            activeAccountType: activeAccount?.type,
            activeOrigin: activeOrigin || undefined,
            pagePath: location.pathname,
            pagePathFragment: `${location.pathname}${location.search}${location.hash}`,
            walletAppMode: isFullScreen ? 'Fullscreen' : 'Pop-up',
            walletVersion: Browser.runtime.getManifest().version,
        });
    }, [activeAccount?.type, activeNetwork, activeOrigin, isFullScreen, location]);

    useEffect(() => {
        ampli.openedWalletExtension();
    }, []);
}
