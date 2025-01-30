// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { useIotaClientContext } from '@iota/dapp-kit';
import {
    StardustIndexerClientContext,
    useStardustIndexerClient,
} from './StardustIndexerClientContext';

export const StardustIndexerClientProvider: React.FC<React.PropsWithChildren> = ({ children }) => {
    const { network } = useIotaClientContext();
    const { stardustIndexerClient } = useStardustIndexerClient(network);

    return (
        <StardustIndexerClientContext.Provider value={{ stardustIndexerClient }}>
            {children}
        </StardustIndexerClientContext.Provider>
    );
};
