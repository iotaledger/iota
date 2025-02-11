// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { useAutoConnectWallet, useCurrentAccount, useCurrentWallet } from '@iota/dapp-kit';
import { redirect, usePathname } from 'next/navigation';
import { PropsWithChildren, useEffect, useState } from 'react';

export function ConnectionGuard({ children }: PropsWithChildren) {
    const [firstLoad, setFirstLoad] = useState<
        'idle' | 'hasStartedConnecting' | 'finishedConnecting'
    >('idle');

    const { isConnecting, isConnected, isDisconnected } = useCurrentWallet();
    const account = useCurrentAccount();
    const pathname = usePathname();
    const autoConnect = useAutoConnectWallet();

    const connected = isConnected && !!account;

    useEffect(() => {
        if (autoConnect === 'idle' && firstLoad === 'idle') {
            return; // wait until first load starts;
        }

        if (isConnecting) {
            setFirstLoad('hasStartedConnecting');
            return;
        }
        setFirstLoad('finishedConnecting');
    }, [isDisconnected, isConnecting, firstLoad, autoConnect]);

    useEffect(() => {
        if (firstLoad !== 'finishedConnecting') return;
        if (!connected && pathname !== '/') {
            redirect('/');
        }
    }, [connected, pathname, firstLoad, isConnecting]);

    return connected || pathname === '/' ? children : null;
}
