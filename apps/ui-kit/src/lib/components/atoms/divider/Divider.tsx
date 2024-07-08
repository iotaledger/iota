// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import { DividerType } from './divider.enums';
import cx from 'classnames';
import { BACKGROUND_COLORS } from './divider.classes';

const DEFAULT_SIZE = '1px';
const FULL_WIDTH = '100%';
interface DividerProps {
    /**
     * The width of the divider.
     */
    width?: string;
    /**
     * The type of the button
     */
    type?: DividerType;
    /**
     * The color of the divider.
     */
    color?: string;
    /**
     * The size of the divider.
     */
    size?: string;
}

export function Divider({
    color,
    width = FULL_WIDTH,
    size = DEFAULT_SIZE,
    type = DividerType.Horizontal,
}: DividerProps): React.JSX.Element {
    // Define the base styles for the divider
    const baseStyle = {
        width: type === DividerType.Horizontal ? width : size,
        height: type === DividerType.Vertical ? width : size,
    };

    // Use classnames to conditionally apply classes based on orientation
    const dividerClasses = cx(BACKGROUND_COLORS);

    return <div className={dividerClasses} style={baseStyle} />;
}
