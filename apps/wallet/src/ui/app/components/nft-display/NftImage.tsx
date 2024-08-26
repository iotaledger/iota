// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { VisualAssetCard, VisualAssetType } from '@iota/apps-ui-kit';

export interface NftImageProps {
    src: string | null;
    video?: string | null;
    name: string | null;
    title?: string;
    playable?: boolean;
    className?: string;
    isLocked?: boolean;
    isHoverable?: boolean;
}

export function NftImage({ src, name, title, isHoverable, video }: NftImageProps) {
    const imgSrc = src ? src.replace(/^ipfs:\/\//, 'https://ipfs.io/ipfs/') : '';

    return video ? (
        <VisualAssetCard
            assetSrc={video}
            assetTitle={title}
            assetType={VisualAssetType.Video}
            altText={name || 'NFT'}
            isHoverable={isHoverable}
        />
    ) : !imgSrc ? (
        <div className="flex aspect-square h-full w-full">
            <span className="text-captionSmall font-medium">No media</span>
        </div>
    ) : (
        <VisualAssetCard
            assetSrc={imgSrc}
            assetTitle={title}
            assetType={VisualAssetType.Image}
            altText={name || 'NFT'}
            isHoverable={isHoverable}
        />
    );
}
