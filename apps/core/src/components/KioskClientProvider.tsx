// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClientContext } from '@iota/dapp-kit';
import { KioskClient } from '@iota/kiosk';
import { createContext, useMemo, type ReactNode } from 'react';

export const KioskClientContext = createContext<KioskClient | null>(null);

export type KioskClientProviderProps = {
    children: ReactNode;
};

export function KioskClientProvider({ children }: KioskClientProviderProps) {
    const { client, network } = useIotaClientContext();
    const kioskClient = useMemo(() => new KioskClient({ client, network }), [client, network]);
    return (
        <KioskClientContext.Provider value={kioskClient}>{children}</KioskClientContext.Provider>
    );
}
