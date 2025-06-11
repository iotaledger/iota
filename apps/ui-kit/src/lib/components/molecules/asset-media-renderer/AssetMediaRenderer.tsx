// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { VisualAssetType } from '../visual-asset-card/visualAssetCard.enums';
import { Video, ImageWithFallback, Image } from '../../atoms';
import type { VideoProps } from '../../atoms';

export interface AssetMediaRendererProps extends VideoProps {
    /**
     * The source of the media to be displayed.
     */
    src: string;
    /**
     * The type of the asset to be displayed.
     */
    assetType: VisualAssetType;
    /**
     * Alt text for the image..
     */
    altText?: string;
    /**
     * Whether to show a fallback image if the main image fails to load.
     */
    showFallback?: boolean;
}

export function AssetMediaRenderer({
    src,
    isAutoPlayEnabled,
    assetType,
    altText: alt,
    showFallback,
    disableControls,
}: AssetMediaRendererProps): React.JSX.Element {
    if (assetType === VisualAssetType.Video) {
        return (
            <Video
                src={src}
                isAutoPlayEnabled={isAutoPlayEnabled}
                disableControls={disableControls}
            />
        );
    }

    if (showFallback) {
        return <ImageWithFallback src={src} alt={alt} />;
    }

    return <Image src={src} alt={alt} />;
}
