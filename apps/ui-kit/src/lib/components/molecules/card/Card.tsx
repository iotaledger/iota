// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import cx from 'classnames';
import { CARD_DISABLED, CARD_CLASSES_VARIANT } from './card.classes';
import { CardVariant } from './card.enums';

export interface CardProps {
    /**
     * If `true`, the card will be disabled.
     */
    disabled?: boolean;

    /**
     * Handler on click to the card.
     */
    onClick?: () => void;

    /**
     * UI variant of the card.
     */
    variant?: CardVariant;

    /**
     * Passing composable Card components like: CardImage, CardText, CardAction.
     */
    children?: React.ReactNode;
}

export function Card({
    disabled = false,
    variant = CardVariant.Default,
    onClick,
    children,
}: CardProps) {
    return (
        <div
            onClick={onClick}
            className={cx(
                'inline-flex items-center gap-3 rounded-lg px-md py-xs',
                CARD_CLASSES_VARIANT[variant].default,
                {
                    [CARD_CLASSES_VARIANT[variant].hover]: !disabled,
                    [CARD_CLASSES_VARIANT[variant].active]: !disabled,
                    [CARD_DISABLED]: disabled,
                },
            )}
        >
            {children}
        </div>
    );
}
