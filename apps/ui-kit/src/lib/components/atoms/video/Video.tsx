// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { VIDEO_AUTOPLAY_FLAGS, VIDEO_AUTOPLAY_FLAGS_NO_CONTROLS } from './video.constants';

export interface VideoProps extends React.ComponentPropsWithoutRef<'video'> {
    /**
     * Ref for the video element.
     */
    ref?: React.Ref<HTMLVideoElement>;
    /**
     * Whether the video should autoplay.
     */
    isAutoPlayEnabled?: boolean;
    /**
     * If the video controls should be disabled.
     */
    disableControls?: boolean;
}

export const Video = ({
    width = '100%',
    height = 'auto',
    isAutoPlayEnabled,
    disableControls,
    ref,
    ...props
}: VideoProps) => {
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
};
Video.displayName = 'Video';
