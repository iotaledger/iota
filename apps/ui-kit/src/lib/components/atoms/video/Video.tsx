// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { forwardRef } from 'react';
import { VIDEO_AUTOPLAY_FLAGS, VIDEO_AUTOPLAY_FLAGS_NO_CONTROLS } from './video.constants';

export interface VideoProps {
    /**
     * Whether the video should autoplay.
     */
    isAutoPlayEnabled?: boolean;
    /**
     * If the video controls should be disabled.
     */
    disableControls?: boolean;
}

export const Video = forwardRef<
    HTMLVideoElement,
    React.ComponentPropsWithoutRef<'video'> & VideoProps
>(({ width = '100%', height = 'auto', isAutoPlayEnabled, disableControls, ...props }, ref) => {
    const videoProps = isAutoPlayEnabled
        ? disableControls
            ? VIDEO_AUTOPLAY_FLAGS_NO_CONTROLS
            : VIDEO_AUTOPLAY_FLAGS
        : { preload: 'metadata', autoPlay: false };

    return (
        <video ref={ref} width={width} height={height} {...videoProps} {...props}>
            Your browser does not support the video tag.
        </video>
    );
});
