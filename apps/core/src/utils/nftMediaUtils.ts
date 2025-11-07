// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { VisualAssetType } from '@iota/apps-ui-kit';
import { transformURL } from './extractMediaFileType';
import { NFTMediaHeadersResult } from '../hooks';

const MAX_VIDEO_SIZE_MB = 50;
const MAX_VIDEO_SIZE_BYTES = MAX_VIDEO_SIZE_MB * 1024 * 1024;

export function isVideoInSizeLimit(contentLength: string | null | undefined): boolean {
    if (contentLength) {
        const sizeInBytes = parseInt(contentLength, 10);
        return sizeInBytes <= MAX_VIDEO_SIZE_BYTES;
    }
    return false;
}

const SUPPORTED_VIDEO_MIMETYPES = ['video/mp4'];
const SUPPORTED_IMAGE_MIMETYPES = [
    'image/jpeg',
    'image/png',
    'image/gif',
    'image/bmp',
    'image/webp',
    'image/x-icon',
    'image/tiff',
    'image/svg+xml',
];

interface ResolveNFTMediaTypeReturn {
    type: VisualAssetType;
    resolvedSrc?: string;
    shouldAutoPlayVideo?: boolean;
    isMediaSupported: boolean;
}

export function resolveNFTMedia(
    src: string | null | undefined,
    nftMediaHeaders: NFTMediaHeadersResult | undefined,
): ResolveNFTMediaTypeReturn {
    const { contentType, contentLength } = nftMediaHeaders || {};
    const urlExtension = getFileExtensionByUrl(src);

    const resolvedSrc = src ? transformURL(src) : undefined;

    if (
        SUPPORTED_VIDEO_MIMETYPES.includes(contentType || '') ||
        SUPPORTED_VIDEO_MIMETYPES.some((mimeType) => mimeType.split('/')[1] === urlExtension)
    ) {
        return {
            type: VisualAssetType.Video,
            resolvedSrc,
            shouldAutoPlayVideo: contentLength ? isVideoInSizeLimit(contentLength) : false,
            isMediaSupported: true,
        };
    }

    if (
        SUPPORTED_IMAGE_MIMETYPES.includes(contentType || '') ||
        SUPPORTED_IMAGE_MIMETYPES.some((mimeType) => mimeType.split('/')[1] === urlExtension)
    ) {
        return {
            type: VisualAssetType.Image,
            resolvedSrc,
            isMediaSupported: true,
        };
    }

    return { type: VisualAssetType.Image, resolvedSrc, isMediaSupported: false };
}

export function getFileExtensionByUrl(src: string | null | undefined): string | undefined {
    if (!src) return undefined;

    try {
        const pathname = new URL(transformURL(src)).pathname;
        const lastSegment = pathname.split('/').pop() || '';
        const cleanSegment = lastSegment.split('?')[0].split('#')[0];
        const ext = cleanSegment.includes('.')
            ? cleanSegment.split('.').pop()?.toLowerCase()
            : undefined;
        return ext;
    } catch {
        return undefined;
    }
}
