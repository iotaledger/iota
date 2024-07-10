// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import { BACKGROUND_COLORS, OUTLINED_BORDER } from './segmented-button.classes';
import cx from 'classnames';
import { ButtonSegment, ButtonSegmentProps } from '../button-segment';
import { SegmentedButtonType } from './segmented-button.enums';
interface SegmentedButtonProps {
    /**
     * The text of the button.
     */
    elements: ButtonSegmentProps[];
    /**
     * The type of the button
     */
    type?: SegmentedButtonType;
    /**
     * The onSelected event of the button.
     */
    onSelected?: (selectedElement: ButtonSegmentProps) => void;
}

export function SegmentedButton({
    elements,
    type = SegmentedButtonType.Filled,
    onSelected,
}: SegmentedButtonProps): React.JSX.Element {
    const backgroundColors = BACKGROUND_COLORS[type];
    const borderColors = type === SegmentedButtonType.Outlined ? OUTLINED_BORDER : '';
    return (
        <div
            className={cx('flex flex-row gap-2 rounded-full p-xxs', backgroundColors, borderColors)}
        >
            {elements.map((element, index) => (
                <ButtonSegment
                    key={index}
                    label={element.label}
                    icon={element.icon}
                    selected={element.selected}
                    disabled={element.disabled}
                    onClick={() => onSelected && onSelected(element)}
                />
            ))}
        </div>
    );
}
