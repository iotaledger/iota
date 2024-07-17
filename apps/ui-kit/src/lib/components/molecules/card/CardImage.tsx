// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import cx from 'classnames';
import { ImagePlaceholder } from '@/components/atoms/image-placeholder';
import { ImageVariant, ImageType } from './card.enums';
import { IMAGE } from './card.classes';

export type CardImageProps = {
    type?: ImageType;
    variant?: ImageVariant;
    url?: string;
    iconName?: string;
};

export function CardImage({ type, variant, url, iconName }: CardImageProps) {
    if (!variant) {
        return null;
    }

    if (type === ImageType.Placeholder) {
        return (
            <div>
                <ImagePlaceholder variant={variant} />
            </div>
        );
    }

    if (type === ImageType.Img && url) {
        return (
            <div className={cx(IMAGE[variant], 'overflow-hidden')}>
                <img src={url} className={cx(IMAGE[variant], 'object-cover')} alt="Card Image" />
            </div>
        );
    }

    if (type === ImageType.Icon && iconName) {
        return (
            <div className={cx(IMAGE[variant], 'overflow-hidden bg-neutral-96')}>
                {/* TODO put icon dynamic */}
            </div>
        );
    }

    if (type === ImageType.IconOnly && iconName) {
        return (
            <div className={cx(IMAGE[variant], 'overflow-hidden')}>
                {/* TODO put icon dynamic */}
            </div>
        );
    }
}
