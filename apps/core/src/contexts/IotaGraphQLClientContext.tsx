// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { useIotaClientContext } from '@iota/dapp-kit';
import { useContext, createContext, useMemo } from 'react';
import { getNetwork } from '@iota/iota-sdk/client';
import { IotaGraphQLClient } from '@iota/iota-sdk/graphql';

export const IotaGraphQLClientProvider: React.FC<React.PropsWithChildren> = ({ children }) => {
    const { network } = useIotaClientContext();
    const { iotaGraphQLClient } = useIotaGraphQLClient(network);

    return (
        <GraphQLClientContext.Provider value={{ iotaGraphQLClient }}>
            {children}
        </GraphQLClientContext.Provider>
    );
};

type IotaGraphQLClientContextType = {
    iotaGraphQLClient: IotaGraphQLClient | null;
};

export const GraphQLClientContext = createContext<IotaGraphQLClientContextType | null>(null);

export function useIotaGraphQLClientContext(): IotaGraphQLClientContextType {
    const context = useContext(GraphQLClientContext);
    if (!context) {
        throw new Error('useGraphQLClientContext must be used within a IotaGraphQLClientProvider');
    }
    return context;
}

export function useIotaGraphQLClient(network?: string) {
    const { graphql } = getNetwork(network || '');

    const iotaGraphQLClient = useMemo(() => {
        if (graphql) {
            return new IotaGraphQLClient({
                url: graphql,
            });
        } else {
            return null;
        }
    }, [graphql]);

    return {
        iotaGraphQLClient,
    };
}
