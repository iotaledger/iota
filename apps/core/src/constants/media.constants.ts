// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { NFTVideoAsset } from '../components';

export const VIDEO_AUTOPLAY_FLAGS: Partial<
    Omit<React.ComponentProps<typeof NFTVideoAsset>, 'src'>
> = {
    autoPlay: true,
    muted: true,
    playsInline: true,
    controls: true,
    controlsList: 'nodownload',
    disablePictureInPicture: true,
    preload: 'metadata',
    loop: true,
    width: '100%',
    height: 'auto',
};
