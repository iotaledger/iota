// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { VisualAssetType } from '@iota/apps-ui-kit';
import { useQuery, UseQueryResult } from '@tanstack/react-query';
import { capitalize, shouldNFTVideoAutoplay, transformURL } from '../utils';

const WHITELISTED_VIDEO_FORMATS = ['mp4'];

const WHITELISTED_IMAGE_MIMETYPES = [
    'image/jpeg',
    'image/png',
    'image/gif',
    'image/bmp',
    'image/webp',
    'image/x-icon',
    'image/tiff',
];

type UseResolveNFTMediaReturnType =
    | {
          assetType: VisualAssetType.Image;
          fileTypeLabel: string;
          src: string;
      }
    | {
          assetType: VisualAssetType.Video;
          isAutoPlayEnabled: boolean;
          fileTypeLabel: string;
          src: string;
      };

export function useResolveNFTMedia(
    src: string | undefined,
): UseQueryResult<UseResolveNFTMediaReturnType> {
    return useQuery({
        queryKey: ['nft-media-info', src],
        queryFn: async ({ signal }) => {
            if (!src) {
                return {
                    assetType: VisualAssetType.Image,
                    fileTypeLabel: '0 Image Files',
                    src: '',
                };
            }

            let assetType: VisualAssetType = VisualAssetType.Image;
            let isAutoPlayEnabled = false;
            let mimeType: string | null = null;
            let mimeTypeSuffix: string | undefined;
            let finalSrc = '';
            const srcExtension = src.split('.').pop()?.toLowerCase();

            try {
                const res = await fetch(transformURL(src), { signal });
                mimeType = res.headers.get('Content-Type');
                const contentLength = res.headers.get('Content-Length');
                mimeTypeSuffix = mimeType?.split('/').pop()?.toLowerCase();

                if (mimeType?.startsWith('video/')) {
                    if (srcExtension && WHITELISTED_VIDEO_FORMATS.includes(srcExtension)) {
                        assetType = VisualAssetType.Video;
                        isAutoPlayEnabled = contentLength
                            ? shouldNFTVideoAutoplay(contentLength)
                            : false;
                        finalSrc = src;
                    }
                } else if (mimeType?.startsWith('image/')) {
                    if (WHITELISTED_IMAGE_MIMETYPES.includes(mimeType)) {
                        assetType = VisualAssetType.Image;
                        finalSrc = src;
                    }
                }
            } catch (_) {
                // fallback to extension
                if (srcExtension && WHITELISTED_VIDEO_FORMATS.includes(srcExtension)) {
                    assetType = VisualAssetType.Video;
                    finalSrc = src;
                } else {
                    assetType = VisualAssetType.Image;
                    finalSrc = ''; // treat as unverified image without Content-Type
                }
                mimeTypeSuffix = srcExtension;
            }

            const mediaType = mimeTypeSuffix ? capitalize(mimeTypeSuffix) : assetType;
            const fileTypeLabel = `1 ${mediaType} File`;

            if (assetType === VisualAssetType.Image) {
                return { assetType, fileTypeLabel, src: finalSrc };
            }

            return {
                assetType,
                isAutoPlayEnabled,
                fileTypeLabel,
                src: finalSrc,
            };
        },
        enabled: !!src,
        refetchOnWindowFocus: false,
        staleTime: 10 * 60 * 1000,
    });
}
