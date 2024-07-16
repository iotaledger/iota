// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { PropsWithChildren } from 'react';
import cx from 'classnames';
import { ArrowRight } from '@iota/ui-icons';
import { Button, ButtonSize, ButtonType } from '@/components';

interface ListItemProps {
    /**
     * Has right icon (optional).
     */
    showRightIcon?: boolean;
    /**
     * On right icon click handler (optional).
     */
    onRightIconClick?: () => void;
    /**
     * The list item is disabled or not.
     */
    disabled?: boolean;
}

export function ListItem({
    showRightIcon,
    onRightIconClick,
    disabled,
    children,
}: PropsWithChildren<ListItemProps>): React.JSX.Element {
    return (
        <div className="w-full border-b border-shader-neutral-light-8 pb-xs dark:border-shader-neutral-dark-8">
            <div
                className={cx(
                    'justif-between relative flex min-h-[48px] flex-row items-center text-neutral-10 dark:text-neutral-92',
                    { 'state-layer': !disabled },
                    { 'opacity-40': disabled },
                )}
            >
                {children}
                {showRightIcon && (
                    <Button
                        size={ButtonSize.Small}
                        type={ButtonType.Ghost}
                        disabled={disabled}
                        onClick={onRightIconClick}
                        icon={<ArrowRight />}
                    />
                )}
            </div>
        </div>
    );
}
