// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { PropsWithChildren } from 'react';
import cx from 'classnames';
import { TooltipPosition } from './tooltip.enums';
import { TOOLTIP_POSITION } from './tooltip.classes';

interface TooltipProps {
    text: string;
    position?: TooltipPosition;
}

export function Tooltip({
    text,
    position = TooltipPosition.Top,
    children,
}: PropsWithChildren<TooltipProps>): React.JSX.Element {
    const tooltipPositionClass = TOOLTIP_POSITION[position];
    return (
        <div className="group relative inline-block cursor-pointer">
            {children}
            <div
                className={cx(
                    'absolute z-50 w-max max-w-[200px] rounded bg-neutral-80 p-xs text-neutral-10 opacity-0 transition-opacity duration-300 group-hover:opacity-100 group-focus:opacity-100 dark:bg-neutral-30 dark:text-neutral-92',
                    tooltipPositionClass,
                )}
                role="tooltip"
            >
                {text}
            </div>
        </div>
    );
}
