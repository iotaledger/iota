// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import cx from 'classnames';
import { Button, ButtonSize, ButtonType } from '../button';
import { ArrowBack, Close } from '@iota/apps-ui-icons';

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
    onClose?: (() => void) | ((e: React.MouseEvent<HTMLElement>) => void);
    /**
     * The 'data-testid' attribute value (used in e2e tests)
     */
    testId?: string;
}

export function Header({
    title,
    titleCentered,
    onBack,
    onClose,
    testId,
}: HeaderProps): JSX.Element {
    const titleCenteredClasses = titleCentered ? 'text-center' : onBack ? 'ml-1' : '';
    const keepSpaceForIcon = titleCentered && (!onBack || !onClose);

    return (
        <div className="header-bg-color header-text-color flex min-h-[56px] w-full items-center px-md--rs pb-xs pt-sm [&_svg]:h-5 [&_svg]:w-5">
            {onBack ? (
                <Button
                    size={ButtonSize.Small}
                    type={ButtonType.Ghost}
                    onClick={onBack}
                    icon={<ArrowBack />}
                    aria-label="Back"
                />
            ) : (
                keepSpaceForIcon && <div className="w-9" />
            )}

            <div className={cx('flex-grow', titleCenteredClasses)}>
                <span className="font-inter text-title-lg" data-testid={testId}>
                    {title}
                </span>
            </div>

            {onClose ? (
                <Button
                    size={ButtonSize.Small}
                    type={ButtonType.Ghost}
                    onClick={onClose}
                    icon={<Close />}
                    testId={`close-icon`}
                    aria-label="Close"
                />
            ) : (
                keepSpaceForIcon && <div className="w-9" />
            )}
        </div>
    );
}
