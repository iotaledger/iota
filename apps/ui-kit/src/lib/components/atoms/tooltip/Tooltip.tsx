// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { PropsWithChildren } from 'react';
import cx from 'classnames';
import { TooltipPosition } from './tooltip.enums';
import { TOOLTIP_POSITION } from './tooltip.classes';

interface TooltipProps {
    text: string;
    position?: TooltipPosition;
    maxWidth?: string;
    usePeers?: boolean;
}

export function Tooltip({
    text,
    position = TooltipPosition.Top,
    maxWidth = 'max-w-[200px]',
    children,
    usePeers = false,
}: PropsWithChildren<TooltipProps>): React.JSX.Element {
    const tooltipPositionClass = TOOLTIP_POSITION[position];
    const commonClasses =
        'absolute z-[999] hidden w-max rounded bg-neutral-80 p-xs text-neutral-10 opacity-0 transition-opacity duration-300 dark:bg-neutral-30 dark:text-neutral-92';

    return usePeers ? (
        <div className="relative inline-block">
            <span className="peer inline-block">{children}</span>
            <div
                className={cx(
                    'peer-hover:block peer-hover:opacity-100 peer-focus:opacity-100 ',
                    tooltipPositionClass,
                    maxWidth,
                    commonClasses,
                )}
                role="tooltip"
            >
                <p className="w-full break-words">{text}</p>
            </div>
        </div>
    ) : (
        <div className="group relative inline-block">
            {children}
            <div
                className={cx(
                    'group-hover:block group-hover:opacity-100 group-focus:opacity-100 ',
                    tooltipPositionClass,
                    maxWidth,
                    commonClasses,
                )}
                role="tooltip"
            >
                <p className="w-full break-words">{text}</p>
            </div>
        </div>
    );
}
