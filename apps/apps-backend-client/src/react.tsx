// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    createContext,
    useCallback,
    useContext,
    useSyncExternalStore,
    type ReactNode,
} from 'react';

import type { AppsBackendClient } from './client';
import type { FeatureResult } from './types';

const AppsBackendClientContext = createContext<AppsBackendClient | null>(null);

interface AppsBackendClientProviderProps {
    client: AppsBackendClient;
    children: ReactNode;
}

export function AppsBackendClientProvider({ client, children }: AppsBackendClientProviderProps) {
    return (
        <AppsBackendClientContext.Provider value={client}>
            {children}
        </AppsBackendClientContext.Provider>
    );
}

export function useAppsBackendClient() {
    const client = useContext(AppsBackendClientContext);
    if (!client) {
        throw new Error('useAppsBackendClient must be used within an AppsBackendClientProvider');
    }
    return client;
}

export function useFeature<T = unknown>(key: string): FeatureResult<T> {
    const client = useAppsBackendClient();

    const subscribe = useCallback((cb: () => void) => client.subscribe(cb), [client]);
    const getSnapshot = useCallback(() => client.getSnapshot(), [client]);

    const features = useSyncExternalStore(subscribe, getSnapshot);
    const feature = features[key];
    const value = (feature?.defaultValue as T) ?? null;

    return {
        value,
        on: !!value,
        off: !value,
    };
}

export function useFeatureValue<T>(key: string, defaultValue: T): T {
    const { value } = useFeature<T>(key);
    return value ?? defaultValue;
}

export function useFeatureIsOn(key: string): boolean {
    const { on } = useFeature(key);
    return on;
}
