// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { forwardRef, useState } from 'react';
import { MediaFallback } from '../media-fallback';

export interface ImageWithFallbackProps extends React.ImgHTMLAttributes<HTMLImageElement> {
    renderFallback?: React.ReactNode;
}

export const ImageWithFallback = forwardRef<HTMLImageElement, ImageWithFallbackProps>(
    ({ onError, renderFallback: fallback, ...imageProps }, ref) => {
        const [imageError, setImageError] = useState(false);

        function handleImageError(error: React.SyntheticEvent<HTMLImageElement, Event>) {
            setImageError(true);
            onError?.(error);
        }

        if (imageError || !imageProps.src) {
            return fallback ? fallback : <MediaFallback />;
        }

        return <Image onError={handleImageError} ref={ref} {...imageProps} />;
    },
);

export const Image = forwardRef<HTMLImageElement, React.ImgHTMLAttributes<HTMLImageElement>>(
    (imageProps, ref) => {
        return <img className="h-full w-full object-cover" ref={ref} {...imageProps} />;
    },
);
