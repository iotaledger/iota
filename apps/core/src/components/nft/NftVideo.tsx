// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';

type NftVideoProps = {
    src: string;
} & Pick<React.HTMLProps<HTMLIFrameElement>, 'width' | 'height'>;

export function NftVideo({ src }: NftVideoProps) {
    return (
        <div className="relative w-full h-full">
            <iframe
                srcDoc={`<!DOCTYPE html>
                        <html>
                            <head>
                                <style>
                                    body {
                                        margin: 0;
                                    }
                                </style>
                            </head>
                            <body>
                                <video
                                    autoplay
                                    muted
                                    playsinline
                                    controls
                                    controlsList="nodownload"
                                    disablePictureInPicture
                                    style="max-width: none; width: 100%; height: 100%; position: absolute; top: 0; left: 0;"
                                >
                                    <source src="${src}" type="video/mp4" />
                                </video>
                            </body>
                        </html>`}
                allowFullScreen
                sandbox="allow-scripts"
                className="border-none overflow-hidden w-full h-full"
                title="video"
            />
        </div>
    );
}
