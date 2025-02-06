// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useContext, createContext, useMemo } from 'react';
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
    const { metadata } = getNetwork<{
        stardustIndexer?: string;
    }>(network || '');

    const stardustIndexerClient = useMemo(() => {
        if (metadata?.stardustIndexer) {
            return new StardustIndexerClient(metadata?.stardustIndexer);
        } else {
            return null;
        }
    }, [metadata?.stardustIndexer]);

    return {
        stardustIndexerClient,
    };
}
