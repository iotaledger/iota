// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { PropsWithChildren, useEffect } from 'react';
import { redirect, usePathname } from 'next/navigation';
import { useAutoConnectWallet, useCurrentWallet } from '@iota/dapp-kit';
import { LoadingIndicator } from '@iota/apps-ui-kit';
import { CONNECT_ROUTE, HOMEPAGE_ROUTE } from '@/lib/constants/routes.constants';

export function ConnectionGuard({ children }: PropsWithChildren) {
    const { isConnected, isDisconnected } = useCurrentWallet();

    const pathname = usePathname();
    const autoConnect = useAutoConnectWallet();

    useEffect(() => {
        if (autoConnect !== 'attempted') return;
        if (isConnected && pathname === CONNECT_ROUTE.path) {
            // Redirect to home if on root ("/")
            redirect(HOMEPAGE_ROUTE.path);
        } else if (isDisconnected && pathname !== CONNECT_ROUTE.path) {
            // Redirect back to "/" if disconnected and trying to access another page
            redirect(CONNECT_ROUTE.path);
        }
    }, [isConnected, isDisconnected, pathname, autoConnect]);

    if (autoConnect === 'idle') {
        return (
            <div className="flex h-screen w-full justify-center">
                <LoadingIndicator size="w-16 h-16" />
            </div>
        );
    }

    return autoConnect === 'attempted' ? children : null;
}
