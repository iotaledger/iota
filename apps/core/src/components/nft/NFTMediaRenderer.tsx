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
}

export function NFTMediaRenderer({
    src,
    alt = 'NFT',
    objectFit = 'object-cover',
    disableVideoControls,
    disableAutoPlay = false,
}: NFTMediaRendererProps) {
    const { isLoading, data: nftMediaHeaders } = useNFTMediaHeaders(src);
    const { resolvedSrc, type, shouldAutoPlayVideo, isMediaSupported } = resolveNFTMedia(
        src,
        nftMediaHeaders,
    );

    const className = clsx('w-full h-full', objectFit);

    if (isLoading) {
        return (
            <div className="flex items-center justify-center h-full w-full">
                <LoadingIndicator />
            </div>
        );
    }

    if (!isMediaSupported) {
        return <MediaFallback />;
    }

    return type === VisualAssetType.Video ? (
        <Video
            src={resolvedSrc}
            isAutoPlayEnabled={!disableAutoPlay && shouldAutoPlayVideo}
            className={className}
            disableControls={disableVideoControls}
        />
    ) : (
        <ImageWithFallback src={resolvedSrc} alt={alt} className={className} />
    );
}
