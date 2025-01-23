// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useContext, createContext, useState, useEffect } from 'react';
import { StardustIndexerClient } from '../';
import { getNetwork } from '@iota/iota-sdk/client';

type StardustIndexerClientContextType = {
    stardustIndexerClient: StardustIndexerClient | null;
};

export const StardustIndexerClientContext = createContext<StardustIndexerClientContextType | null>(
    null,
);

export function useStardustIndexerClientContext(): StardustIndexerClientContextType {
    const context = useContext(StardustIndexerClientContext);
    if (!context) {
        throw new Error('useStardustIndexerClient must be used within a StardustClientProvider');
    }
    return context;
}

export function useStardustIndexerClient(network?: string) {
    const [stardustIndexerClient, setStardustIndexerClient] =
        useState<StardustIndexerClient | null>(null);

    const { stardustIndexer } = getNetwork(network || '');

    useEffect(() => {
        if (!stardustIndexer) {
            setStardustIndexerClient(null);
        } else {
            setStardustIndexerClient(new StardustIndexerClient(stardustIndexer));
        }
    }, [stardustIndexer]);

    return {
        stardustIndexerClient,
    };
}
