// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import cx from 'classnames';
import { Button, ButtonSize, ButtonType } from '../button';
import { ArrowLeft, Close } from '@iota/ui-icons';

interface HeaderProps {
    /**
     * Header title.
     */
    title: string;
    /**
     * Title alignment (optional).
     */
    titleCentered?: boolean;
    /**
     * On back click handler (optional). If provided, a back button will be displayed.
     */
    onBack?: () => void;
    /**
     * On close click handler (optional). If provided, a close button will be displayed.
     */
    onClose?: () => void;
}

export function Header({ title, titleCentered, onBack, onClose }: HeaderProps): JSX.Element {
    const titleCenteredClasses = titleCentered ? 'text-center' : onBack ? 'ml-1' : '';
    const keepSpaceForIcon = titleCentered && (!onBack || !onClose);

    return (
        <div className="flex min-h-[56px] w-full items-center bg-neutral-100 px-md--rs pb-xs pt-sm text-neutral-10 dark:bg-neutral-6 dark:text-neutral-92">
            {onBack ? (
                <Button
                    size={ButtonSize.Small}
                    type={ButtonType.Ghost}
                    onClick={onBack}
                    icon={<ArrowLeft className="h-5 w-5" />}
                />
            ) : (
                keepSpaceForIcon && <div className="w-9" />
            )}

            <div className={cx('flex-grow', titleCenteredClasses)}>
                <span className="font-inter text-title-lg">{title}</span>
            </div>

            {onClose ? (
                <Button
                    size={ButtonSize.Small}
                    type={ButtonType.Ghost}
                    onClick={onClose}
                    icon={<Close className="h-5 w-5" />}
                />
            ) : (
                keepSpaceForIcon && <div className="w-9" />
            )}
        </div>
    );
}
