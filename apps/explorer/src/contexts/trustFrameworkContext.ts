// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import type { IdentityClientReadOnly } from '@iota/identity-wasm/web';
import type { NotarizationClientReadOnly } from '@iota/notarization/web';
import { createContext, useContext } from 'react';

export interface TrustFrameworkProviderContext {
    identityClient: IdentityClientReadOnly | null;
    notarizationClient: NotarizationClientReadOnly | null;
}

export const TrustFrameworkContext = createContext<TrustFrameworkProviderContext>({
    identityClient: null,
    notarizationClient: null,
});

export function useTrustFramework(): TrustFrameworkProviderContext {
    const context = useContext(TrustFrameworkContext);

    if (!context) {
        throw new Error('useTrustFramework must be used within a TrustFrameworkProvider');
    }

    return context;
}

export function useIdentityClient(): IdentityClientReadOnly | null {
    return useTrustFramework().identityClient;
}

export function useNotarizationClient(): NotarizationClientReadOnly | null {
    return useTrustFramework().notarizationClient;
}

export function useIdentityPkgId(): string | null {
    return useIdentityClient()?.packageId() || null;
}

export function useNotarizationPkgId(): string | null {
    return useNotarizationClient()?.packageId() || null;
}
