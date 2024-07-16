// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import cx from 'classnames';
import { AccountType } from './account.enums';
import { BACKGROUND_BADGE_COLORS, TEXT_COLORS, BADGE_TEXT_CLASS } from './account.classes';
import { Button, ButtonSize, ButtonType } from '../../atoms/button';
import { Address } from '../../atoms';
import { LockLocked, LockUnlocked, MoreHoriz } from '@iota/ui-icons';

interface AccountProps {
    /**
     * The title of the account.
     */
    title: string;
    /**
     * The subtitle of the account.
     */
    subtitle: string;
    /**
     * The type of the account.
     */
    accountType?: AccountType | null;
    /**
     * Whether the account is unlocked.
     */
    isLocked?: boolean;
    /**
     * Handler for the three dots icon click.
     */
    onOptionsClick: () => void;
    /**
     * Handler for the lock account icon click.
     */
    onLockAccountClick: () => void;
    /**
     * Handle for the unlock account icon click.
     */
    onUnlockAccountClick: () => void;
    /**
     * Function to render avatar content.
     */
    avatarContent: ({ isLocked }: { isLocked?: boolean }) => React.JSX.Element;
    /**
     * Handler for the onCopy event in Address component.
     */
    onCopy?: (e: React.MouseEvent<HTMLButtonElement>) => void;
    /**
     * Handler for the onOpen event in Address component.
     */
    onOpen?: (e: React.MouseEvent<HTMLButtonElement>) => void;
    /**
     * Has copy icon (optional).
     */
    isCopyable?: boolean;
    /**
     * Has open icon  (optional).
     */
    isExternal?: boolean;
}

export function Account({
    title,
    subtitle,
    accountType,
    isLocked,
    avatarContent,
    onOptionsClick,
    onLockAccountClick,
    onUnlockAccountClick,
    onCopy,
    onOpen,
    isCopyable,
    isExternal,
}: AccountProps): React.JSX.Element {
    const backgroundBadgeClasses = accountType ? BACKGROUND_BADGE_COLORS[accountType] : '';
    const textClasses = accountType ? TEXT_COLORS[accountType] : '';
    const Avatar = avatarContent;

    return (
        <div className="state-layer group relative flex w-full items-center justify-between space-x-3 rounded-xl px-sm py-xs hover:cursor-pointer">
            <div className="flex items-center space-x-3">
                <Avatar isLocked={isLocked} />
                <div className="flex flex-col items-start py-xs">
                    <div className="flex items-center space-x-2">
                        <span className="text-title-md text-neutral-10 dark:text-neutral-92">
                            {title}
                        </span>
                        {accountType && (
                            <div
                                className={cx(
                                    'flex items-center rounded-full px-xs py-xxxs',
                                    backgroundBadgeClasses,
                                )}
                            >
                                <div className={cx(BADGE_TEXT_CLASS, textClasses)}>
                                    {accountType}
                                </div>
                            </div>
                        )}
                    </div>
                    <Address
                        text={subtitle}
                        onCopy={onCopy}
                        onOpen={onOpen}
                        isCopyable={isCopyable}
                        isExternal={isExternal}
                    />
                </div>
            </div>
            <div
                className={cx(
                    'z-10 ml-auto flex items-center space-x-2 [&_button]:hidden group-hover:[&_button]:flex',
                    isLocked && '[&_button:last-child]:flex',
                )}
            >
                <Button
                    size={ButtonSize.Small}
                    type={ButtonType.Ghost}
                    onClick={onOptionsClick}
                    icon={<MoreHoriz />}
                />
                {isLocked ? (
                    <Button
                        size={ButtonSize.Small}
                        type={ButtonType.Ghost}
                        onClick={onUnlockAccountClick}
                        icon={<LockLocked />}
                    />
                ) : (
                    <Button
                        size={ButtonSize.Small}
                        type={ButtonType.Ghost}
                        onClick={onLockAccountClick}
                        icon={<LockUnlocked />}
                    />
                )}
            </div>
        </div>
    );
}
