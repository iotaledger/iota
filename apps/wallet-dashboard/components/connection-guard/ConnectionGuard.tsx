// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { useCurrentAccount, useCurrentWallet } from '@iota/dapp-kit';
import { redirect, usePathname } from 'next/navigation';
import { PropsWithChildren, useEffect } from 'react';

export function ConnectionGuard({ children }: PropsWithChildren) {
    const { connectionStatus } = useCurrentWallet();
    const account = useCurrentAccount();
    const pathname = usePathname();

    const connected = connectionStatus === 'connected' && account;
    useEffect(() => {
        if (!connected && pathname !== '/') {
            redirect('/');
        }
    }, [connected, pathname]);

    return connected || pathname === '/' ? children : null;
}
