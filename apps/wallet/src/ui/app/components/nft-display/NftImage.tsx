// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { VisualAssetCard, VisualAssetType } from '@iota/apps-ui-kit';

export interface NftImageProps {
    src: string | null;
    video?: string | null;
    title?: string;
    className?: string;
    isHoverable?: boolean;
    icon?: React.ReactNode;
    onIconClick?: (e: React.MouseEvent<HTMLButtonElement>) => void;
}

export function NftImage({ src, title, isHoverable, video, icon, onIconClick }: NftImageProps) {
    const imgSrc = src ? src.replace(/^ipfs:\/\//, 'https://ipfs.io/ipfs/') : '';

    if (video) {
        return (
            <VisualAssetCard
                assetSrc={video}
                assetTitle={title}
                assetType={VisualAssetType.Video}
                altText={title || 'NFT'}
                isHoverable={isHoverable}
                icon={icon}
                onIconClick={onIconClick}
            />
        );
    }
    if (!imgSrc) {
        return (
            <div className="flex aspect-square h-full w-full items-center justify-center">
                <span className="text-captionSmall font-medium">No media</span>
            </div>
        );
    }

    return (
        <VisualAssetCard
            assetSrc={imgSrc}
            assetTitle={title}
            assetType={VisualAssetType.Image}
            altText={title || 'NFT'}
            isHoverable={isHoverable}
            icon={icon}
            onIconClick={onIconClick}
        />
    );
}
