// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import cx from 'classnames';
import { CARD_DISABLED_CLASSES, CARD_TYPE_CLASSES } from './card.classes';
import { CardType } from './card.enums';

export interface CardProps {
    /**
     * If `true`, the card will be disabled.
     */
    isDisabled?: boolean;

    /**
     * Handler on click to the card.
     */
    onClick?: () => void;

    /**
     * UI variant of the card.
     */
    type?: CardType;

    /**
     * Passing composable Card components like: CardImage, CardText, CardAction.
     */
    children?: React.ReactNode;
}

export function Card({
    isDisabled = false,
    type = CardType.Default,
    onClick,
    children,
}: CardProps) {
    return (
        <div
            onClick={onClick}
            className={cx(
                'relative inline-flex w-full items-center gap-3 rounded-xl px-sm py-xs',
                CARD_TYPE_CLASSES[type],
                {
                    'state-layer': !isDisabled,
                    [CARD_DISABLED_CLASSES]: isDisabled,
                    'cursor-pointer': onClick,
                },
            )}
        >
            {children}
        </div>
    );
}
