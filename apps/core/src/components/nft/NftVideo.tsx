// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { forwardRef } from 'react';

export const NftVideo = forwardRef<HTMLVideoElement, React.ComponentPropsWithoutRef<'video'>>(
    ({ width = '100%', height = 'auto', className, ...props }, ref) => {
        return (
            <video ref={ref} {...props} width={width} height={height} className={className}>
                Your browser does not support the video tag.
            </video>
        );
    },
);
