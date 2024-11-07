// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { useCurrentAccount, useCurrentWallet } from '@iota/dapp-kit';
import { redirect } from 'next/navigation';
import { PropsWithChildren, useEffect } from 'react';

export function ConnectionGuard({ children }: PropsWithChildren) {
    const { connectionStatus } = useCurrentWallet();
    const account = useCurrentAccount();

    const connected = connectionStatus === 'connected' && account;

    useEffect(() => {
        if (!connected) {
            redirect('/');
        }
    }, [connected]);

    return connected ? children : null;
}
