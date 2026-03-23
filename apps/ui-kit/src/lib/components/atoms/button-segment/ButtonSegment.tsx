// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    BACKGROUND_COLORS,
    BACKGROUND_COLORS_SELECTED,
    TEXT_COLORS,
    TEXT_COLORS_SELECTED,
    UNDERLINED,
    UNDERLINED_SELECTED,
} from './buttonSegment.classes';
import cx from 'classnames';
import { ButtonSegmentType } from './buttonSegment.enums';
import { ButtonUnstyled } from '../button';

interface ButtonSegmentProps extends React.AriaAttributes {
    /**
     * The label of the button.
     */
    label: string;
    /**
     The icon of the button
     */
    icon?: React.ReactNode;
    /**
     The selected flag of the button
     */
    selected?: boolean;
    /**
     * The button is disabled or not.
     */
    disabled?: boolean;
    /**
     * The onClick event of the button.
     */
    onClick?: (e: React.MouseEvent<HTMLButtonElement>) => void;
    /**
     * The type of the button.
     */
    type?: ButtonSegmentType;
    /**
     * If the button is nested inside a parent button.
     */
    isNested?: boolean;
}

export function ButtonSegment({
    icon,
    label,
    selected,
    disabled,
    onClick,
    type = ButtonSegmentType.Rounded,
    isNested = false,
    ...ariaProps
}: ButtonSegmentProps): React.JSX.Element {
    const isUnderlined = type === ButtonSegmentType.Underlined;

    const backgroundColors = selected
        ? isUnderlined
            ? 'bg-transparent'
            : BACKGROUND_COLORS_SELECTED
        : BACKGROUND_COLORS;

    const underlined = isUnderlined ? (selected ? UNDERLINED_SELECTED : UNDERLINED) : '';
    const textColors = selected ? TEXT_COLORS_SELECTED : TEXT_COLORS;
    const padding = isUnderlined ? (isNested ? 'px-md py-sm' : 'px-lg py-md') : 'px-sm py-[6px]';
    const borderRadius = isUnderlined ? '' : 'rounded-full';
    const textSize = isNested
        ? isUnderlined
            ? 'text-title-sm'
            : 'text-label-md'
        : isUnderlined
          ? 'text-title-md'
          : 'text-label-lg';

    return (
        <ButtonUnstyled
            onClick={onClick}
            className={cx(
                'enabled:state-layer-secondary relative flex items-center disabled:opacity-40',
                backgroundColors,
                textColors,
                padding,
                borderRadius,
                underlined,
                {
                    'pl-xs': !!icon && !isUnderlined,
                },
            )}
            disabled={disabled}
            {...ariaProps}
        >
            <div className={cx('flex flex-row items-center justify-center gap-2', textSize)}>
                {icon && <span>{icon}</span>}
                <span className="text-start font-inter">{label}</span>
            </div>
        </ButtonUnstyled>
    );
}
