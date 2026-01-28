// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { useMemo } from 'react';
import { EvmRpcClientContext } from '../contexts';
import { EvmRpcClient } from '@iota/isc-sdk';

export type EvmRpcClientProviderProps = {
    baseUrl: string;
};
export const EvmRpcClientProvider: React.FC<React.PropsWithChildren<EvmRpcClientProviderProps>> = ({
    children,
    baseUrl,
}) => {
    const evmRpcClient = useMemo(() => {
        return new EvmRpcClient(baseUrl);
    }, [baseUrl]);

    return (
        <EvmRpcClientContext.Provider value={{ evmRpcClient }}>
            {children}
        </EvmRpcClientContext.Provider>
    );
};
