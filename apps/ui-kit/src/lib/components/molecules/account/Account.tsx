// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import cx from 'classnames';
import { ButtonUnstyled } from '../../atoms/button';
import { Badge, BadgeType } from '../../atoms';
import { LockLocked, LockUnlocked, MoreHoriz, CheckmarkFilled } from '@iota/ui-icons';
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
    onOptionsClick?: (e: React.MouseEvent<HTMLButtonElement>) => void;
    /**
     * Handler for the lock account icon click.
     */
    onLockAccountClick?: (e: React.MouseEvent<HTMLButtonElement>) => void;
    /**
     * Handle for the unlock account icon click.
     */
    onUnlockAccountClick?: (e: React.MouseEvent<HTMLButtonElement>) => void;
    /**
     * Function to render avatar content.
     */
    avatarContent: ({ isLocked }: { isLocked?: boolean }) => React.JSX.Element;
    /**
     * The onCopy event of the Address  (optional).
     */
    onCopy?: (e: React.MouseEvent<HTMLButtonElement>) => void;
    /**
     * Text that need to be copied (optional).
     */
    copyText?: string;
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
     * The account is selected.
     */
    isSelected?: boolean;
    /**
     * Show the selected checkbox.
     */
    showSelected?: boolean;
    /**
     * Show background if account active (optional).
     */
    isActive?: boolean;
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
    copyText,
    onOpen,
    isCopyable,
    isExternal,
    isSelected,
    isActive,
    showSelected,
}: AccountProps): React.JSX.Element {
    const Avatar = avatarContent;

    return (
        <div
            className={cx(
                'group relative flex w-full items-center justify-between space-x-3 rounded-xl px-sm py-xs hover:cursor-pointer',
                isActive && 'state-active',
                {
                    'opacity-60': isLocked,
                    'state-layer': !isLocked,
                },
            )}
        >
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
                        copyText={copyText}
                        onOpen={onOpen}
                        isCopyable={isCopyable}
                        isExternal={isExternal}
                    />
                </div>
            </div>
            <div
                className={cx(
                    'z-10 ml-auto flex items-center space-x-2 [&_button]:hidden [&_button]:h-5 [&_button]:w-5 group-hover:[&_button]:flex',
                    '[&_svg]:h-5 [&_svg]:w-5 [&_svg]:text-neutral-40',
                    '[&_div.checkmark_button]:flex', // make checkmark visible always
                    isLocked && '[&_div.unlock_button]:flex', // make unlock visible when is locked
                )}
            >
                <div className="flex items-center space-x-2 [&_button]:hidden group-hover:[&_button]:flex [&_svg]:text-neutral-40 [&_svg]:dark:text-neutral-60">
                    {onOptionsClick && (
                        <ButtonUnstyled onClick={onOptionsClick}>
                            <MoreHoriz />
                        </ButtonUnstyled>
                    )}
                    {onLockAccountClick &&
                        onUnlockAccountClick &&
                        (isLocked ? (
                            <div className="unlock">
                                <ButtonUnstyled onClick={onUnlockAccountClick}>
                                    <LockLocked />
                                </ButtonUnstyled>
                            </div>
                        ) : (
                            <ButtonUnstyled onClick={onLockAccountClick}>
                                <LockUnlocked />
                            </ButtonUnstyled>
                        ))}
                </div>
                {showSelected && (
                    <div className="checkmark">
                        <ButtonUnstyled>
                            <CheckmarkFilled
                                className={cx({
                                    'text-neutral-10': !isSelected,
                                    'text-primary-30': isSelected,
                                })}
                            />
                        </ButtonUnstyled>
                    </div>
                )}
            </div>
        </div>
    );
}
