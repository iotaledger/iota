// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

import { useCallback } from 'react';

export function getBackendUrl() {
    if (
        typeof import.meta !== 'undefined' &&
        // eslint-disable-next-line @typescript-eslint/ban-ts-comment
        // @ts-ignore
        import.meta.env &&
        // eslint-disable-next-line @typescript-eslint/ban-ts-comment
        // @ts-ignore
        import.meta.env.VITE_APPS_BACKEND_URL
    ) {
        // eslint-disable-next-line @typescript-eslint/ban-ts-comment
        // @ts-ignore
        return import.meta.env.VITE_APPS_BACKEND_URL;
    }

    return process.env.NEXT_PUBLIC_APPS_BACKEND_URL;
}

export function useAppsBackend() {
    const backendUrl = getBackendUrl();

    const request = useCallback(
        async <T>(
            path: string,
            queryParams?: Record<string, any>,
            options?: RequestInit,
        ): Promise<T> => {
            const res = await fetch(
                formatRequestURL(`${backendUrl}/${path}`, queryParams),
                options,
            );

            if (!res.ok) {
                throw new Error('Unexpected response');
            }

            return res.json();
        },
        [],
    );

    return { request };
}

function formatRequestURL(url: string, queryParams?: Record<string, any>) {
    if (queryParams && Object.keys(queryParams).length > 0) {
        const searchParams = new URLSearchParams(queryParams);
        return `${url}?${searchParams}`;
    }
    return url;
}
