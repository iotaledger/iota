// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { createContext, useContext, useMemo, type ReactNode } from 'react';

import { IotaNamesIndexerClient } from '@/auctions/services/IotaNamesIndexerClient';
import { CONFIG } from '@/config';

export const IotaNamesIndexerClientContext = createContext<IotaNamesIndexerClient | null>(null);

export interface IotaNamesIndexerClientProviderProps {
    children: ReactNode;
}

export function IotaNamesIndexerClientProvider({ children }: IotaNamesIndexerClientProviderProps) {
    const client = useMemo(() => {
        return new IotaNamesIndexerClient(CONFIG.indexerUrl);
    }, []);

    return (
        <IotaNamesIndexerClientContext.Provider value={client}>
            {children}
        </IotaNamesIndexerClientContext.Provider>
    );
}

export function useIotaNamesIndexerClientContext(): IotaNamesIndexerClient | null {
    const context = useContext(IotaNamesIndexerClientContext);
    if (context === undefined) {
        throw new Error(
            'useIotaNamesIndexerClientContext must be used within an IotaNamesIndexerClientProvider',
        );
    }
    return context;
}
