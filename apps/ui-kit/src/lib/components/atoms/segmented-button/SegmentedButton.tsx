// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { useState } from 'react';
import { BACKGROUND_COLORS } from './segmented-button.classes';
import cx from 'classnames';
import { ButtonSegment } from '../button-segment';
import { SegmentedButtonType } from './segmented-button.enums';

interface SegmentedButtonProps {
    /**
     * The text of the button.
     */
    elements: { label: string; icon?: React.ReactNode; disabled?: boolean }[];
    /**
     * The type of the button
     */
    type?: SegmentedButtonType;
    /**
     * The onSelected event of the button.
     */
    onSelected?: (selectedElement: { label: string }) => void;
}

export function SegmentedButton({
    elements,
    onSelected,
    type = SegmentedButtonType.Filled,
}: SegmentedButtonProps): React.JSX.Element {
    const [selected, setSelected] = useState<string | null>(null);
    const backgroundColors = BACKGROUND_COLORS[type];

    const handleButtonClick = (
        element: { label: string; icon?: React.ReactNode; disabled?: boolean },
        index: number,
    ) => {
        if (element.disabled || element.label === selected) {
            // Do nothing if the button is disabled or already selected
            return;
        }
        const newSelected = element.label;
        setSelected(newSelected);
        onSelected?.(element);
    };
    return (
        <div className={cx('flex flex-row gap-2 rounded-full p-xxs', backgroundColors)}>
            {elements.map((element, index) => (
                <ButtonSegment
                    key={index}
                    label={element.label}
                    icon={element.icon}
                    selected={element.label === selected}
                    onClick={() => handleButtonClick(element, index)}
                />
            ))}
        </div>
    );
}
