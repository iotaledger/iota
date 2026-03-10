// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
'use client';

import { LoadingIndicator } from '@iota/apps-ui-kit';
import { useAutoConnectWallet } from '@iota/dapp-kit';
import { PropsWithChildren } from 'react';

export function ConnectionGuard({ children }: PropsWithChildren) {
    const autoConnect = useAutoConnectWallet();

    if (autoConnect === 'idle') {
        return (
            <div className="flex h-screen w-full justify-center">
                <LoadingIndicator size="w-16 h-16" />
            </div>
        );
    }

    return autoConnect === 'attempted' ? children : null;
}
