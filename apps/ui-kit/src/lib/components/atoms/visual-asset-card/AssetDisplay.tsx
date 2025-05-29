// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { VisualAssetType } from './visualAssetCard.enums';
import { PlaceholderReplace } from '@iota/apps-ui-icons';
import { forwardRef, useState } from 'react';

export interface AssetDisplayProps {
    /**
     * The type of the asset to be displayed.
     */
    assetType?: VisualAssetType;
    /**
     * The source of the image to be displayed.
     */
    src: string;
    /**
     * Alt text for the image.
     */
    altText: string;
}

export function AssetDisplay({ src, assetType, altText }: AssetDisplayProps): React.JSX.Element {
    return assetType === VisualAssetType.Video ? (
        <video src={src} className="h-full w-full object-cover" autoPlay loop muted />
    ) : (
        <ImageWithFallback src={src} alt={altText} />
    );
}

interface ImageWithFallbackProps extends React.ImgHTMLAttributes<HTMLImageElement> {
    fallback?: React.ReactNode;
    forceFallback?: boolean;
}

export const ImageWithFallback = forwardRef<HTMLImageElement, ImageWithFallbackProps>(
    ({ onError, fallback, forceFallback, ...imageProps }, ref) => {
        const [imageError, setImageError] = useState(false);

        function handleImageError(error: React.SyntheticEvent<HTMLImageElement, Event>) {
            setImageError(true);
            onError?.(error);
        }

        const shouldFallback = (forceFallback && fallback) || imageError;

        if (shouldFallback) {
            return !fallback ? (
                <div className="flex h-full w-full items-center justify-center bg-neutral-96 dark:bg-neutral-10">
                    <PlaceholderReplace className="h-4 w-4 text-neutral-40 dark:text-neutral-60" />
                </div>
            ) : (
                fallback
            );
        }

        return (
            <img
                className="h-full w-full object-cover"
                onError={handleImageError}
                ref={ref}
                {...imageProps}
            />
        );
    },
);
