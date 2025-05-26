// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { VisualAssetCard, VisualAssetType } from '@iota/apps-ui-kit';
import { NftVideo } from './NftVideo';

export interface NftMediaDisplayProps {
    imageSrc?: string | null;
    videoSrc?: string | null;
    title?: string;
    className?: string;
    isHoverable?: boolean;
    icon?: React.ReactNode;
    onIconClick?: (e: React.MouseEvent<HTMLButtonElement>) => void;
}

export function NftMediaDisplay({
    imageSrc,
    videoSrc,
    title,
    isHoverable,
    icon,
    onIconClick,
}: NftMediaDisplayProps) {
    const imgSrc = imageSrc ? imageSrc.replace(/^ipfs:\/\//, 'https://ipfs.io/ipfs/') : '';

    const mediaProps: React.ComponentProps<typeof VisualAssetCard> = videoSrc
        ? { renderAsset: <NftVideo src={videoSrc} /> }
        : { assetSrc: imgSrc, assetType: VisualAssetType.Image, altText: title || 'NFT' };

    return (
        <VisualAssetCard
            {...mediaProps}
            altText={title || 'NFT'}
            isHoverable={isHoverable}
            icon={icon}
            onIconClick={onIconClick}
        />
    );
}
