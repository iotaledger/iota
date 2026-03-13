// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { PropsWithChildren } from 'react';
import cx from 'classnames';
import { ArrowRight } from '@iota/apps-ui-icons';
import { Button, ButtonSize, ButtonType } from '../button';

export interface ListItemProps extends React.AriaAttributes {
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
    /**
     * The list item is highlighted.
     */
    isHighlighted?: boolean;
}

export function ListItem({
    showRightIcon,
    hideBottomBorder,
    onClick,
    isDisabled,
    children,
    isHighlighted,
    ...ariaProps
}: PropsWithChildren<ListItemProps>): React.JSX.Element {
    function handleKeyDown(event: React.KeyboardEvent<HTMLDivElement>) {
        if ((event.key === 'Enter' || event.key === ' ') && !isDisabled && onClick) {
            onClick();
        }
    }

    function handleClick() {
        if (!isDisabled && onClick) {
            onClick();
        }
    }

    return (
        <div
            className={cx(
                'w-full',
                {
                    'list-item-border-color border-b pb-xs': !hideBottomBorder,
                },
                { 'opacity-40': isDisabled },
            )}
        >
            <div
                onClick={handleClick}
                role="button"
                tabIndex={0}
                onKeyDown={handleKeyDown}
                {...ariaProps}
                className={cx(
                    'list-item-color relative flex flex-row items-center justify-between px-md py-sm',
                    !isDisabled && onClick ? 'cursor-pointer' : 'cursor-default',
                    {
                        'list-item-highlight-bg': isHighlighted,
                        'state-layer-secondary': !isDisabled,
                    },
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
