// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { VisualAssetType } from '@iota/apps-ui-kit';
import { useQuery, UseQueryResult } from '@tanstack/react-query';
import { capitalize, shouldNFTVideoAutoplay, transformURL } from '../utils';

const ALLOWED_VIDEO_EXTENSIONS = ['mp4'];

type UseResolveNFTMediaReturnType =
    | {
          assetType: VisualAssetType.Image;
          fileTypeLabel: string;
      }
    | {
          assetType: VisualAssetType.Video;
          isAutoPlayEnabled: boolean;
          fileTypeLabel: string;
      };

export function useResolveNFTMedia(src: string): UseQueryResult<UseResolveNFTMediaReturnType> {
    return useQuery({
        queryKey: ['nft-media-info', src],
        queryFn: async ({ signal }) => {
            if (!src) {
                return {
                    assetType: VisualAssetType.Image,
                    fileTypeLabel: '0 Image Files',
                };
            }

            let assetType: VisualAssetType = VisualAssetType.Image;
            let isAutoPlayEnabled = false;
            let mimeTypeSuffix: string | undefined;

            try {
                const res = await fetch(transformURL(src), { signal });
                const contentType = res.headers.get('Content-Type');
                const contentLength = res.headers.get('Content-Length');

                mimeTypeSuffix = contentType?.split('/').pop()?.toLowerCase();

                if (contentType?.startsWith('video/')) {
                    assetType = VisualAssetType.Video;
                    isAutoPlayEnabled = contentLength
                        ? shouldNFTVideoAutoplay(contentLength)
                        : false;
                } else if (contentType?.startsWith('image/')) {
                    assetType = VisualAssetType.Image;
                }
            } catch (_) {
                // fallback to extension
                const ext = src.split('.').pop()?.toLowerCase();
                mimeTypeSuffix = ext;

                if (ALLOWED_VIDEO_EXTENSIONS.includes(ext || '')) {
                    assetType = VisualAssetType.Video;
                } else {
                    assetType = VisualAssetType.Image;
                }
            }

            const mediaType = mimeTypeSuffix ? capitalize(mimeTypeSuffix) : assetType;
            const fileTypeLabel = `1 ${mediaType} File`;

            if (assetType === VisualAssetType.Image) {
                return { assetType, fileTypeLabel };
            }

            return {
                assetType,
                isAutoPlayEnabled,
                fileTypeLabel,
            };
        },
        enabled: !!src,
        refetchOnWindowFocus: false,
        staleTime: 10 * 60 * 1000,
    });
}
