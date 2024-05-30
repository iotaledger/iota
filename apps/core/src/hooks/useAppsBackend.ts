// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

import { getAppsBackend } from '@mysten/sui.js/client';
import { useCallback } from 'react';

export function useAppsBackend() {
    const backendUrl = getAppsBackend();

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
