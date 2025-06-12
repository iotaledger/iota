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

interface NFTMediaRendererProps {
    src: string;
    alt?: string;
    disableVideoControls?: boolean;
    disableAutoPlay?: boolean;
}

export function NFTMediaRenderer({
    src,
    alt = 'NFT',
    disableVideoControls,
    disableAutoPlay = false,
}: NFTMediaRendererProps) {
    const { isLoading, data: nftMediaHeaders } = useNFTMediaHeaders(src);
    const { type, shouldAutoPlayVideo, isMediaSupported } = resolveNFTMedia(src, nftMediaHeaders);

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
            src={src}
            isAutoPlayEnabled={!disableAutoPlay && shouldAutoPlayVideo}
            className="w-full h-full object-cover"
            disableControls={disableVideoControls}
        />
    ) : (
        <ImageWithFallback src={src} alt={alt} className="w-full h-full object-cover" />
    );
}
