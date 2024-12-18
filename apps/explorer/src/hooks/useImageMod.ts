// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useAppsBackend } from '@iota/core';
import { type UseQueryResult, useQuery } from '@tanstack/react-query';
import { ImageVisibility } from '~/lib/enums';

// https://cloud.google.com/vision/docs/supported-files
const SUPPORTED_IMG_TYPES = [
    'image/jpeg',
    'image/png',
    'image/gif',
    'image/bmp',
    'image/webp',
    'image/x-icon',
    'application/pdf',
    'image/tiff',
];

type ImageModeration = {
    visibility?: ImageVisibility;
};

const placeholderData = {
    visibility: ImageVisibility.Pass,
};

const isURL = (url?: string) => {
    if (!url) return false;
    try {
        new URL(url);
        return true;
    } catch (e) {
        return false;
    }
};

export function useImageMod({
    url = '',
    enabled = true,
}: {
    url?: string;
    enabled?: boolean;
}): UseQueryResult<ImageModeration | undefined, Error> {
    const { request } = useAppsBackend();

    return useQuery<ImageModeration | undefined, Error>({
        queryKey: ['image-mod', url, enabled],
        queryFn: async () => {
            if (!isURL(url) || !enabled) return placeholderData;

            const res = await fetch(url, {
                method: 'HEAD',
            });

            const contentType = res.headers.get('Content-Type');

            if (contentType && SUPPORTED_IMG_TYPES.includes(contentType)) {
                return request<ImageModeration>('image', {
                    url,
                });
            }
        },
        placeholderData,
        staleTime: 24 * 60 * 60 * 1000,
        gcTime: Infinity,
    });
}
