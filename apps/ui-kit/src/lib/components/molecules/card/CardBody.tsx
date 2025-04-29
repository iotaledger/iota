// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import cx from 'classnames';

import { ReactNode } from 'react';
import { Tooltip, TooltipPosition } from '../../atoms';

export type CardBodyProps = {
    title: string;
    subtitle?: string | ReactNode;
    clickableAction?: React.ReactNode;
    icon?: React.ReactNode;
    isTextTruncated?: boolean;
    tooltipText?: string;
    tooltipPosition?: TooltipPosition;
};

export function CardBody({
    title,
    subtitle,
    clickableAction,
    icon,
    isTextTruncated,
    tooltipText,
    tooltipPosition,
}: CardBodyProps) {
    const handleActionCardBodyClick = (event: React.MouseEvent) => {
        event?.stopPropagation();
    };
    return (
        <div
            className={cx('flex w-full flex-col', {
                truncate: isTextTruncated,
            })}
        >
            <div
                className={cx('flex flex-row items-center gap-x-xxs', {
                    'grow-1': isTextTruncated,
                })}
            >
                <div
                    className={cx('font-inter text-title-md text-neutral-10 dark:text-neutral-92', {
                        truncate: isTextTruncated,
                    })}
                >
                    {title}
                </div>
                {tooltipText ? (
                    <Tooltip text={tooltipText} position={tooltipPosition}>
                        {icon}
                    </Tooltip>
                ) : (
                    <div className="flex items-center">{icon}</div>
                )}
                {clickableAction && (
                    <div onClick={handleActionCardBodyClick} className="flex items-center">
                        {clickableAction}
                    </div>
                )}
            </div>
            {subtitle && (
                <div
                    className={cx('font-inter text-body-md text-neutral-40 dark:text-neutral-60', {
                        truncate: isTextTruncated,
                    })}
                >
                    {subtitle}
                </div>
            )}
        </div>
    );
}
