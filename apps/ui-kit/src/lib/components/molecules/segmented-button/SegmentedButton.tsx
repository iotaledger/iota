// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { PropsWithChildren } from 'react';
import { BACKGROUND_COLORS, OUTLINED_BORDER } from './segmented-button.classes';
import cx from 'classnames';
import { SegmentedButtonType } from './segmented-button.enums';

interface SegmentedButtonProps {
    /**
     * The type of the button
     */
    type?: SegmentedButtonType;
}

export function SegmentedButton({
    type = SegmentedButtonType.Filled,
    children,
}: PropsWithChildren<SegmentedButtonProps>): React.JSX.Element {
    const backgroundColors = BACKGROUND_COLORS[type];
    const borderColors = type === SegmentedButtonType.Outlined ? OUTLINED_BORDER : '';
    return (
        <div
            className={cx('flex flex-row gap-1 rounded-full p-xxs', backgroundColors, borderColors)}
        >
            {children}
        </div>
    );
}
