// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { forwardRef } from 'react';
import { VIDEO_AUTOPLAY_FLAGS } from '../../constants';

interface NFTVideoAssetProps {
    isAutoPlayEnabled?: boolean;
}

export const NFTVideoAsset = forwardRef<
    HTMLVideoElement,
    React.ComponentPropsWithoutRef<'video'> & NFTVideoAssetProps
>(({ width = '100%', height = 'auto', className, isAutoPlayEnabled, ...props }, ref) => {
    const videoProps = isAutoPlayEnabled
        ? VIDEO_AUTOPLAY_FLAGS
        : { preload: 'metadata', autoPlay: false };

    return (
        <video
            ref={ref}
            width={width}
            height={height}
            className={className}
            {...videoProps}
            {...props}
        >
            Your browser does not support the video tag.
        </video>
    );
});
