// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { useIotaClientContext } from '@iota/dapp-kit';
import { IotaNamesClient } from '@iota/iota-names-sdk';
import { getNetwork } from '@iota/iota-sdk/client';
import { IotaGraphQLClient } from '@iota/iota-sdk/graphql';
import React, { createContext, useContext, useMemo } from 'react';

export const IotaNamesClientProvider: React.FC<React.PropsWithChildren> = ({ children }) => {
    const { iotaNamesClient } = useIotaNamesClient();

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

export function useIotaNamesClientContext(): IotaNamesClientContextType {
    const context = useContext(IotaNamesClientContext);

    if (!context) {
        throw new Error('useIotaNamesClientContext must be used within a IotaNamesClientProvider');
    }

    return context;
}

export function useIotaNamesClient() {
    const ctx = useIotaClientContext();
    const network = getNetwork(ctx.network);

    const iotaNamesClient = useMemo(() => {
        return new IotaNamesClient({
            graphQlClient: new IotaGraphQLClient({
                url: network.graphql!,
            }),
            network: network.id,
        });
    }, [network]);

    return { iotaNamesClient };
}
