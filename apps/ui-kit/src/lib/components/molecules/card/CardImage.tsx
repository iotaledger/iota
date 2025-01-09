// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import cx from 'classnames';
import { ImageType, ImageShape } from './card.enums';
import { IMAGE_BG_CLASSES, IMAGE_VARIANT_CLASSES } from './card.classes';
import { CardImagePlaceholder } from './CardImagePlaceholder';

export interface CardImageProps {
    type?: ImageType;
    shape?: ImageShape;
    url?: string;
    children?: React.ReactNode;
}

export function CardImage({
    type = ImageType.BgSolid,
    shape = ImageShape.Rounded,
    url,
    children,
}: CardImageProps) {
    return (
        <div
            className={cx(
                IMAGE_VARIANT_CLASSES[shape],
                IMAGE_BG_CLASSES[type],
                'flex shrink-0 items-center justify-center  overflow-hidden',
            )}
        >
            {type === ImageType.Placeholder && !children && (
                <CardImagePlaceholder variant={shape} />
            )}
            {url && !children && (
                <img
                    src={url}
                    className={cx(IMAGE_VARIANT_CLASSES[shape], 'object-cover')}
                    alt="Card Image"
                />
            )}
            {children}
        </div>
    );
}
