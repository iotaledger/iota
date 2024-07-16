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
    disabled: boolean;
    cardVariant: CardVariant;
    children: React.ReactNode;
} & CardTextProps &
    CardImageProps &
    CardActionProps;

export function Card({
    disabled = false,
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

    const CARD_HOVER = `hover:cursor-pointer hover:bg-primary-60 hover:bg-opacity-8`;
    const CARD_ACTIVE = `active:cursor-pointer active:bg-primary-60 active:bg-opacity-12`;
    const CARD_DISABLED = `cursor-default opacity-40`;

    return (
        <div
            // onClick={}
            className={cx(
                'inline-flex items-center gap-3 rounded-lg px-md py-xs',
                CARD_CLASSES_VARIANT[cardVariant],
                !disabled && CARD_HOVER,
                !disabled && CARD_ACTIVE,
                disabled && CARD_DISABLED,
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
