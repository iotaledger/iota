// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import { DividerType } from './divider.enums';
import cx from 'classnames';
import { BACKGROUND_COLORS, DIVIDER_FULL_WIDTH } from './divider.classes';

const DEFAULT_LINE_HEIGHT = '1px';

interface DividerProps {
    /**
     * The type of the button
     */
    type?: DividerType;
    /**
     * The width of the divider.
     */
    width?: string;
    /**
     * The height of the divider.
     */
    height?: string;
    /**
     * The line height of the divider.
     */
    lineHight?: string;
}

export function Divider({
    type = DividerType.Horizontal,
    width,
    height,
    lineHight = DEFAULT_LINE_HEIGHT,
}: DividerProps): React.JSX.Element {
    // Set width and height of divider line based on type
    const baseStyle = {
        ...(type === DividerType.Horizontal ? { height: lineHight } : { width: lineHight }),
    };

    let dividerSize = DIVIDER_FULL_WIDTH[type];
    if (width && type === DividerType.Horizontal) {
        dividerSize = width;
    } else if (height && type === DividerType.Vertical) {
        dividerSize = height;
    }

    return <div className={cx(BACKGROUND_COLORS, dividerSize)} style={baseStyle} />;
}
