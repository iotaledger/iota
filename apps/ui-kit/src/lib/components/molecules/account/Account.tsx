// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import cx from 'classnames';
import { Button, ButtonSize, ButtonType } from '../../atoms/button';
import { Badge, BadgeType } from '../../atoms';
import { LockLocked, LockUnlocked, MoreHoriz } from '@iota/ui-icons';
import { Address } from '../address';

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
     * Whether the account is unlocked.
     */
    isLocked?: boolean;
    /**
     * Handler for more options click.
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
     * The onCopy event of the Address  (optional).
     */
    onCopy?: (e: React.MouseEvent<HTMLButtonElement>) => void;
    /**
     * The onOpen event of the Address  (optional).
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
    /**
     * The type of the badge.
     */
    badgeType?: BadgeType;
    /**
     * The text of the badge.
     */
    badgeText?: string;
}

export function Account({
    title,
    subtitle,
    badgeType,
    badgeText,
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
    const Avatar = avatarContent;

    return (
        <div className="state-layer group relative flex w-full items-center justify-between space-x-3 rounded-xl px-sm py-xs hover:cursor-pointer">
            <div className="flex items-center space-x-3">
                <Avatar isLocked={isLocked} />
                <div className="flex flex-col items-start py-xs">
                    <div className="flex items-center space-x-2">
                        <span className="font-inter text-title-md text-neutral-10 dark:text-neutral-92">
                            {title}
                        </span>
                        {badgeType && badgeText && <Badge type={badgeType} label={badgeText} />}
                    </div>
                    <Address
                        text={subtitle}
                        onCopySuccess={onCopy}
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
