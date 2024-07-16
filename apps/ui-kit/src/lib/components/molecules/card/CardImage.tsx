// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ImagePlaceholder } from '@/components/atoms/image-placeholder';
import cx from 'classnames';

export enum ImageType {
    None = 'none',
    Placeholder = 'placeholder',
    Img = 'img',
    Icon = 'icon',
    IconOnly = 'icon-only',
}

export enum ImageVariant {
    Rounded = 'rounded',
    SquareRounded = 'square-rounded',
}

export type CardImageProps = {
    imageType?: ImageType;
    imageVariant?: ImageVariant;
    imageUrl?: string;
    iconName?: string;
};

export function CardImage({ imageType, imageVariant, imageUrl, iconName }: CardImageProps) {
    const IMAGE_SIZE = 'h-[40px] w-[40px]';

    const IMAGE: { [key in ImageVariant]: string } = {
        [ImageVariant.SquareRounded]: `${IMAGE_SIZE} rounded-md`,
        [ImageVariant.Rounded]: `${IMAGE_SIZE} rounded-full`,
    };

    if (!imageVariant) {
        return null;
    }

    if (imageType === ImageType.Placeholder) {
        return (
            <div>
                <ImagePlaceholder variant={imageVariant} />
            </div>
        );
    }

    if (imageType === ImageType.Img && imageUrl) {
        return (
            <div className={cx(IMAGE[imageVariant], 'overflow-hidden')}>
                <img
                    src={imageUrl}
                    className={cx(IMAGE[imageVariant], 'object-cover')}
                    alt="Card Image"
                />
            </div>
        );
    }

    if (imageType === ImageType.Icon && iconName) {
        return (
            <div className={cx(IMAGE[imageVariant], 'overflow-hidden bg-neutral-96')}>
                {/* TODO put icon dynamic */}
            </div>
        );
    }

    if (imageType === ImageType.IconOnly && iconName) {
        return (
            <div className={cx(IMAGE[imageVariant], 'overflow-hidden')}>
                {/* TODO put icon dynamic */}
            </div>
        );
    }
}
