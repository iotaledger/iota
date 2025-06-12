// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    MediaFallback,
    ImageWithFallback,
    LoadingIndicator,
    VisualAssetType,
    Video,
} from '@iota/apps-ui-kit';
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
    const { type, shouldAutoPlayVideo, isMediaSupported } = resolveNFTMedia(src, nftMediaHeaders);

    const className = clsx('w-full h-full', objectFit);

    if (isLoading) {
        return <LoadingIndicator />;
    }

    if (!isMediaSupported) {
        return <MediaFallback />;
    }

    return type === VisualAssetType.Video ? (
        <Video
            src={src}
            isAutoPlayEnabled={!disableAutoPlay && shouldAutoPlayVideo}
            className={className}
            disableControls={disableVideoControls}
            ref={videoRef}
        />
    ) : (
        <ImageWithFallback src={src} alt={alt} ref={imageRef} className={className} />
    );
}
