// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { VisualAssetType } from './visualAssetCard.enums';
import { PlaceholderReplace } from '@iota/apps-ui-icons';
import { useState } from 'react';

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
    const [imageError, setImageError] = useState(false);

    return assetType === VisualAssetType.Video ? (
        <video src={src} className="h-full w-full object-cover" autoPlay loop muted />
    ) : imageError ? (
        <div className="flex h-full w-full items-center justify-center bg-neutral-96 dark:bg-neutral-10">
            <PlaceholderReplace className="h-4 w-4 text-neutral-40 dark:text-neutral-60" />
        </div>
    ) : (
        <img
            src={src}
            alt={altText}
            className="h-full w-full object-cover"
            onError={() => setImageError(true)}
        />
    );
}

export function ImageDisplayWithFallback({
    src,
    altText,
}: Omit<AssetDisplayProps, 'assetType'>): React.JSX.Element {
    const [imageError, setImageError] = useState(false);

    if (imageError) {
        return (
            <div className="flex h-full w-full items-center justify-center bg-neutral-96 dark:bg-neutral-10">
                <PlaceholderReplace className="h-4 w-4 text-neutral-40 dark:text-neutral-60" />
            </div>
        );
    }

    return (
        <img
            src={src}
            alt={altText}
            className="h-full w-full object-cover"
            onError={() => setImageError(true)}
        />
    );
}
