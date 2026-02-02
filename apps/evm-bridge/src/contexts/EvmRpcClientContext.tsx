// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { EvmRpcClient } from '@iota/isc-sdk';
import { useContext, createContext } from 'react';

type EvmRpcClientContextType = {
    evmRpcClient: EvmRpcClient | null;
};

export const EvmRpcClientContext = createContext<EvmRpcClientContextType | null>(null);

export function useEvmRpcClient(): EvmRpcClientContextType {
    const context = useContext(EvmRpcClientContext);
    if (!context) {
        throw new Error('useEvmRpcClient must be used within a EvmRpcClientProvider');
    }
    return context;
}
