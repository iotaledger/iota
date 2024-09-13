// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { PropsWithChildren } from 'react';
import cx from 'classnames';
import { ArrowRight } from '@iota/ui-icons';
import { Button, ButtonSize, ButtonType } from '@/components';

export interface ListItemProps {
    /**
     * Has right icon (optional).
     */
    showRightIcon?: boolean;
    /**
     * Hide bottom border (optional).
     */
    hideBottomBorder?: boolean;
    /**
     * On click handler (optional).
     */
    onClick?: () => void;
    /**
     * The list item is disabled or not.
     */
    isDisabled?: boolean;
}

export function ListItem({
    showRightIcon,
    hideBottomBorder,
    onClick,
    isDisabled,
    children,
}: PropsWithChildren<ListItemProps>): React.JSX.Element {
    function handleKeyDown(event: React.KeyboardEvent<HTMLDivElement>) {
        if ((event.key === 'Enter' || event.key === ' ') && !isDisabled) {
            onClick && onClick();
        }
    }

    function handleClick() {
        if (!isDisabled) {
            onClick && onClick();
        }
    }

    return (
        <div
            className={cx(
                'w-full',
                {
                    'border-b border-shader-neutral-light-8 pb-xs dark:border-shader-neutral-dark-8':
                        !hideBottomBorder,
                },
                { 'opacity-40': isDisabled },
            )}
        >
            <div
                onClick={handleClick}
                role="button"
                tabIndex={0}
                onKeyDown={handleKeyDown}
                className={cx(
                    'relative flex flex-row items-center justify-between px-md py-sm text-neutral-10 dark:text-neutral-92',
                    !isDisabled && onClick ? 'cursor-pointer' : 'cursor-default',
                    { 'state-layer': !isDisabled },
                )}
            >
                {children}
                {showRightIcon && (
                    <Button
                        size={ButtonSize.Small}
                        type={ButtonType.Ghost}
                        disabled={isDisabled}
                        icon={<ArrowRight />}
                    />
                )}
            </div>
        </div>
    );
}
