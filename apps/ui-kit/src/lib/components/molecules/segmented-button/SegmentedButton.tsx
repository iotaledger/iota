// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { PropsWithChildren } from 'react';
import { BACKGROUND_COLORS, OUTLINED_BORDER } from './segmentedButton.classes';
import cx from 'classnames';
import { SegmentedButtonType } from './segmentedButton.enums';
import { ButtonSegmentType } from '../../atoms';

interface SegmentedButtonProps {
    /**
     * The type of the button
     */
    type?: SegmentedButtonType;
    /**
     * The shape of the button
     */
    shape?: ButtonSegmentType;
}

export function SegmentedButton({
    type = SegmentedButtonType.Filled,
    children,
    shape = ButtonSegmentType.Rounded,
}: PropsWithChildren<SegmentedButtonProps>): React.JSX.Element {
    const backgroundColors = BACKGROUND_COLORS[type];
    const borderColors = type === SegmentedButtonType.Outlined ? OUTLINED_BORDER : '';
    const borderShape = shape === ButtonSegmentType.Rounded ? 'rounded-full gap-1 p-xxs' : '';
    return (
        <div className={cx('flex flex-row flex-wrap', backgroundColors, borderColors, borderShape)}>
            {children}
        </div>
    );
}
