// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import cx from 'classnames';
import { CARD_DISABLED, CARD_CLASSES_VARIANT } from './card.classes';
import { CardVariant } from './card.enums';

export type CardProps = {
    disabled?: boolean;
    onClick?: () => void;
    variant?: CardVariant;
    children?: React.ReactNode;
};

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
