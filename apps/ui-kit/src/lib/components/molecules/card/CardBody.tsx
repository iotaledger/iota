// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import cx from 'classnames';

export type CardBodyProps = {
    title: string;
    subtitle?: string;
    clickableAction?: React.ReactNode;
    isTruncateText?: boolean;
};

export function CardBody({ title, subtitle, clickableAction, isTruncateText }: CardBodyProps) {
    const handleActionCardBodyClick = (event: React.MouseEvent) => {
        event?.stopPropagation();
    };
    return (
        <div
            className={cx('flex w-full flex-col', {
                'grow-1 overflow-hidden': isTruncateText,
            })}
        >
            <div
                className={cx('flex flex-row gap-x-xs', {
                    'grow-1': isTruncateText,
                })}
            >
                <div
                    className={cx('font-inter text-title-md text-neutral-10 dark:text-neutral-92', {
                        'grow-1 overflow-hidden text-ellipsis whitespace-nowrap': isTruncateText,
                    })}
                >
                    {title}
                </div>
                {clickableAction && (
                    <div onClick={handleActionCardBodyClick}>{clickableAction}</div>
                )}
            </div>
            {subtitle && (
                <div
                    className={cx('font-inter text-body-md text-neutral-40 dark:text-neutral-60', {
                        'grow-1 overflow-hidden text-ellipsis whitespace-nowrap': isTruncateText,
                    })}
                >
                    {subtitle}
                </div>
            )}
        </div>
    );
}
