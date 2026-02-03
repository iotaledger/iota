// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useState } from 'react';
import { MediaFallback } from '../media-fallback';

export interface ImageWithFallbackProps extends React.ImgHTMLAttributes<HTMLImageElement> {
    ref?: React.Ref<HTMLImageElement>;
    renderFallback?: React.ReactNode;
}

export const ImageWithFallback = ({
    onError,
    renderFallback: fallback,
    ref,
    ...imageProps
}: ImageWithFallbackProps) => {
    const [imageError, setImageError] = useState(false);

    function handleImageError(error: React.SyntheticEvent<HTMLImageElement, Event>) {
        setImageError(true);
        onError?.(error);
    }

    if (imageError || !imageProps.src) {
        return fallback ? fallback : <MediaFallback />;
    }

    return <Image onError={handleImageError} ref={ref} {...imageProps} />;
};
ImageWithFallback.displayName = 'ImageWithFallback';

export interface ImageProps extends React.ImgHTMLAttributes<HTMLImageElement> {
    ref?: React.Ref<HTMLImageElement>;
}

export const Image = ({ ref, ...imageProps }: ImageProps) => {
    return <img className="h-full w-full object-cover" ref={ref} {...imageProps} />;
};
Image.displayName = 'Image';
