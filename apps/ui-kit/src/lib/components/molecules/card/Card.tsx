// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import cx from 'classnames';
import { CardAction, CardActionProps } from './CardAction';
import { CardText, CardTextProps } from './CardText';
import { CardImage, CardImageProps } from './CardImage';
import { CARD_HOVER, CARD_ACTIVE, CARD_DISABLED } from './card.classes';
import { CardVariant } from './card.enums';

type CardProps = {
    disabled?: boolean;
    onClick?: () => void;
    cardVariant?: CardVariant;
    children?: React.ReactNode;
    text?: CardTextProps;
    image?: CardImageProps;
    action?: CardActionProps;
};

export function Card({
    disabled = false,
    cardVariant = CardVariant.Default,
    onClick,
    text,
    image,
    action,
    children,
}: CardProps) {
    const CARD_CLASSES_VARIANT = {
        [CardVariant.Default]: '',
        [CardVariant.Outlined]: 'border border-shader-neutral-light-8 p-xs',
        [CardVariant.Filled]: 'bg-shader-neutral-light-8 p-xs',
    };

    return (
        <div
            onClick={onClick}
            className={cx(
                'inline-flex items-center gap-3 rounded-lg px-md py-xs',
                CARD_CLASSES_VARIANT[cardVariant],
                !disabled && CARD_HOVER,
                !disabled && CARD_ACTIVE,
                disabled && CARD_DISABLED,
            )}
        >
            <CardImage {...image} />
            <CardText {...text} />
            <CardAction {...action} />
            {children}
        </div>
    );
}
