// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export const VIDEO_AUTOPLAY_FLAGS: Partial<
    Omit<React.VideoHTMLAttributes<HTMLVideoElement>, 'src'>
> = {
    autoPlay: true,
    muted: true,
    playsInline: true,
    controls: true,
    controlsList: 'nodownload',
    disablePictureInPicture: true,
    preload: 'auto',
    loop: true,
    width: '100%',
    height: 'auto',
};

export const VIDEO_AUTOPLAY_FLAGS_NO_CONTROLS: Partial<
    Omit<React.VideoHTMLAttributes<HTMLVideoElement>, 'src'>
> = {
    autoPlay: true,
    muted: true,
    playsInline: true,
    controls: false,
    disablePictureInPicture: true,
    preload: 'auto',
    loop: true,
    width: '100%',
    height: 'auto',
};
