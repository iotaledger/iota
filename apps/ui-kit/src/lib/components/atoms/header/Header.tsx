// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Close, Back } from '@iota/icons';
import cx from 'classnames';

interface HeaderProps {
    /**
     * Header title.
     */
    title: string;
    /**
     * Has icon to the left of the text (optional).
     */
    hasLeftIcon?: boolean;
    /**
     * Has icon to the right of the text (optional).
     */
    hasRightIcon?: boolean;
    /**
     * Title alignment (optional).
     */
    titleCentered?: boolean;
    /**
     * On back click handler (optional).
     */
    onBack?: () => void;
    /**
     * On close click handler (optional).
     */
    onClose?: () => void;
}

export function Header({
    title,
    hasLeftIcon,
    hasRightIcon,
    titleCentered,
    onBack,
    onClose,
}: HeaderProps): JSX.Element {
    const titleCenteredClasses = titleCentered ? 'text-center' : 'ml-1';
    const iconSizeClass = 'h-5 w-5 shrink-0';
    return (
        <div className="flex min-h-[56px] w-full items-center bg-neutral-100 px-lg pb-xs pt-sm text-neutral-10 dark:bg-neutral-6 dark:text-neutral-92">
            {hasLeftIcon && (
                <div className="p-xs">
                    <Back className={cx(iconSizeClass)} onClick={onBack} />
                </div>
            )}

            <div className={cx('flex-grow', titleCenteredClasses)}>
                <span className="text-title-lg">{title}</span>
            </div>

            {hasRightIcon && (
                <div className="p-xs">
                    <Close className={cx(iconSizeClass)} onClick={onClose} />
                </div>
            )}
        </div>
    );
}
