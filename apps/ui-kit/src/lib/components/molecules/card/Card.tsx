// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import cx from 'classnames';
import { CardAction, CardActionProps, CardActionVariant } from './CardAction';
import { CardText, CardTextProps } from './CardText';
import { CardImage, CardImageProps, ImageVariant, ImageType } from './CardImage';

enum CardVariant {
    Default = 'default',
    Outlined = 'outlined',
    Filled = 'filled',
}

type CardProps = {
    cardVariant: CardVariant;
    children: React.ReactNode;
} & CardTextProps &
    CardImageProps &
    CardActionProps;

export function Card({
    cardVariant = CardVariant.Default,
    title,
    subtitle,
    imageType = ImageType.Placeholder,
    imageVariant = ImageVariant.Rounded,
    imageUrl,
    iconName,
    actionTitle,
    actionSubtitle,
    actionOnClick,
    actionVariant = CardActionVariant.SupportingText,
    children,
}: CardProps) {
    const CARD_CLASSES_VARIANT = {
        [CardVariant.Default]: 'border border-shader-neutral-light-8 p-xs',
        [CardVariant.Outlined]: 'border border-shader-neutral-light-8 p-xs',
        [CardVariant.Filled]: 'bg-shader-neutral-light-8 p-xs',
    };

    return (
        <div
            className={cx(
                'inline-flex items-center gap-3 rounded-lg px-md py-xs',
                CARD_CLASSES_VARIANT[cardVariant],
            )}
        >
            <CardImage
                imageType={imageType}
                imageVariant={imageVariant}
                imageUrl={imageUrl}
                iconName={iconName}
            />
            <CardText title={title} subtitle={subtitle} />
            <CardAction
                actionVariant={actionVariant}
                actionOnClick={actionOnClick}
                actionTitle={actionTitle}
                actionSubtitle={actionSubtitle}
            />
            {children}
        </div>
    );
}
