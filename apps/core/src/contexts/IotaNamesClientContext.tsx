// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { useIotaClientContext } from '@iota/dapp-kit';
import { IotaNamesClient } from '@iota/iota-names-sdk';
import { getNetwork } from '@iota/iota-sdk/client';
import React, { createContext, useContext, useMemo } from 'react';
import { useIotaGraphQLClientContext } from './IotaGraphQLClientContext';

export const IotaNamesClientProvider: React.FC<React.PropsWithChildren> = ({ children }) => {
    const ctx = useIotaClientContext();
    const network = getNetwork(ctx.network);
    const { iotaGraphQLClient } = useIotaGraphQLClientContext();

    // The GraphQL client is too expensive to memoize
    // but we know for sure it will only be recreated when the network changes
    const iotaNamesClient = useMemo(() => {
        if (!iotaGraphQLClient) return null;

        return new IotaNamesClient({
            graphQlClient: iotaGraphQLClient,
            network: network.id,
        });
    }, [network.id]);

    return (
        <IotaNamesClientContext.Provider value={{ iotaNamesClient }}>
            {children}
        </IotaNamesClientContext.Provider>
    );
};

type IotaNamesClientContextType = {
    iotaNamesClient: IotaNamesClient | null;
};

export const IotaNamesClientContext = createContext<IotaNamesClientContextType | null>(null);

export function useIotaNamesClient(): IotaNamesClientContextType {
    const context = useContext(IotaNamesClientContext);

    if (!context) {
        throw new Error('useIotaNamesClient must be used within a IotaNamesClientProvider');
    }

    return context;
}
