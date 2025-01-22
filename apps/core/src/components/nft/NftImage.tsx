// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { CardImage, ImageType, VisualAssetCard, VisualAssetType } from '@iota/apps-ui-kit';
import { PlaceholderReplace } from '@iota/apps-ui-icons';
import cx from 'clsx';

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
            <div
                className={cx(
                    'relative flex aspect-square h-full w-full items-center justify-center overflow-hidden rounded-xl',
                    {
                        'group cursor-pointer': isHoverable,
                    },
                )}
            >
                {isHoverable && (
                    <div className="absolute left-0 top-0 h-full w-full bg-cover bg-center bg-no-repeat group-hover:bg-shader-neutral-light-48 group-hover:transition group-hover:duration-300 group-hover:ease-in-out group-hover:dark:bg-shader-primary-dark-48" />
                )}
                <CardImage type={ImageType.BgTransparent}>
                    <PlaceholderReplace className="text-neutral-40" />
                </CardImage>
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
