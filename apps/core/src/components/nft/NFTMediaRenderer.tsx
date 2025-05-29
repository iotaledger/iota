// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ImageDisplayWithFallback, VisualAssetType } from '@iota/apps-ui-kit';
import { NFTVideoAsset } from './NFTVideoAsset';
import { Loader } from '@iota/apps-ui-icons';
import { useResolveNFTMedia } from '../../hooks';

interface NFTMediaRendererProps {
    src: string;
    alt?: string;
    width?: string | number;
    height?: string | number;
}

export function NFTMediaRenderer({ src, alt = 'NFT' }: NFTMediaRendererProps) {
    const { data: resolvedNFTInfo, isLoading } = useResolveNFTMedia(src);

    return resolvedNFTInfo?.assetType === VisualAssetType.Video ? (
        !isLoading ? (
            <NFTVideoAsset
                src={src}
                isAutoPlayEnabled={resolvedNFTInfo.isAutoPlayEnabled}
                className="w-full h-full object-cover"
            />
        ) : (
            <Loader className="w-full h-full flex items-center justify-center" />
        )
    ) : (
        <ImageDisplayWithFallback src={src} altText={alt} />
    );
}
