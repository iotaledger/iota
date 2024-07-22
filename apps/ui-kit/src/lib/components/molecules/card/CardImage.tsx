// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import cx from 'classnames';
import { ImageType, ImageVariant } from './card.enums';
import { IMAGE_BG_CLASSES, IMAGE_VARIANT_CLASSES } from './card.classes';
import { CardImagePlaceholder } from './CardImagePlaceholder';

export interface CardImageProps {
    type?: ImageType;
    variant?: ImageVariant;
    url?: string;
    children?: React.ReactNode;
}

export function CardImage({
    type = ImageType.BgSolid,
    variant = ImageVariant.Rounded,
    url,
    children,
}: CardImageProps) {
    if (!variant) {
        return null;
    }

    return (
        <div
            className={cx(
                IMAGE_VARIANT_CLASSES[variant],
                IMAGE_BG_CLASSES[type],
                'flex items-center justify-center overflow-hidden',
            )}
        >
            {type === ImageType.Placeholder && !children && (
                <CardImagePlaceholder variant={variant} />
            )}
            {url && !children && (
                <img
                    src={url}
                    className={cx(IMAGE_VARIANT_CLASSES[variant], 'object-cover')}
                    alt="Card Image"
                />
            )}
            {children}
        </div>
    );
}
