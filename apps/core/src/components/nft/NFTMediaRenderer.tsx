// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ImageWithFallback, LoadingIndicator, VisualAssetType } from '@iota/apps-ui-kit';
import { NFTVideoAsset } from './NFTVideoAsset';
import { useNFTMediaHeaders } from '../../hooks';
import { resolveNFTMedia } from '../../utils';
import clsx from 'clsx';

interface NFTMediaRendererProps {
    src: string;
    alt?: string;
    disableVideoControls?: boolean;
    disableAutoPlay?: boolean;
    objectFit?: string | null;
    imageRef?: React.Ref<HTMLImageElement>;
    videoRef?: React.Ref<HTMLVideoElement>;
}

export function NFTMediaRenderer({
    src,
    alt = 'NFT',
    objectFit = 'object-cover',
    disableVideoControls,
    disableAutoPlay = false,
    imageRef,
    videoRef,
}: NFTMediaRendererProps) {
    const { isLoading, data: nftMediaHeaders } = useNFTMediaHeaders(src);
    const { type, isAutoPlaySupported, showFallback } = resolveNFTMedia(src, nftMediaHeaders);

    const className = clsx('w-full h-full', objectFit);

    if (isLoading) {
        return <LoadingIndicator />;
    }

    return type === VisualAssetType.Video ? (
        <NFTVideoAsset
            src={src}
            isAutoPlayEnabled={!disableAutoPlay && isAutoPlaySupported}
            className={className}
            disableControls={disableVideoControls}
            ref={videoRef}
        />
    ) : (
        <ImageWithFallback
            src={src}
            alt={alt}
            forceFallback={showFallback}
            ref={imageRef}
            className={className}
        />
    );
}
