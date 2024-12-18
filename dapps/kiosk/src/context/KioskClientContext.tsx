// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient, useIotaClientContext } from '@iota/dapp-kit';
import { KioskClient, Network } from '@iota/kiosk';
import { createContext, ReactNode, useContext, useMemo } from 'react';

export const KioskClientContext = createContext<KioskClient | undefined>(undefined);

export function KioskClientProvider({ children }: { children: ReactNode }) {
    const iotaClient = useIotaClient();
    const { network } = useIotaClientContext();
    const kioskClient = useMemo(
        () =>
            new KioskClient({
                client: iotaClient,
                network: network as Network,
            }),
        [iotaClient, network],
    );

    return (
        <KioskClientContext.Provider value={kioskClient}>{children}</KioskClientContext.Provider>
    );
}

export function useKioskClient() {
    const kioskClient = useContext(KioskClientContext);
    if (!kioskClient) {
        throw new Error('kioskClient not setup properly.');
    }
    return kioskClient;
}
