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
    return (
        <div
            className={cx(
                'w-full pb-xs',
                {
                    'border-b border-shader-neutral-light-8 dark:border-shader-neutral-dark-8':
                        !hideBottomBorder,
                },
                { 'opacity-40': isDisabled },
            )}
        >
            <div
                onClick={onClick}
                className={cx(
                    'relative flex min-h-[48px] flex-row items-center justify-between px-md text-neutral-10 dark:text-neutral-92',
                    { 'state-layer': !isDisabled, 'cursor-pointer': !isDisabled && onClick },
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
