// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import cx from 'classnames';
import type { BadgeType } from '@/components/atoms';
import { Badge, Tooltip, TooltipPosition, ButtonUnstyled } from '@/components/atoms';
import {
    LockLocked,
    LockUnlocked,
    MoreHoriz,
    CheckmarkFilled,
    RadioOff,
} from '@iota/apps-ui-icons';
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
    /**
     * The tooltip text of the badge.
     */
    badgeTooltipText?: string;
    /**
     * The tooltip position of the badge.
     */
    badgeTooltipPosition?: TooltipPosition;
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
    badgeTooltipText,
    badgeTooltipPosition = TooltipPosition.Bottom,
}: AccountProps): React.JSX.Element {
    const Avatar = avatarContent;

    return (
        <div
            data-testid="account-tile"
            className={cx(
                'state-layer group relative flex w-full items-center justify-between space-x-3 rounded-xl px-sm py-xs hover:cursor-pointer',
                isActive && 'state-active',
                {
                    'opacity-60': isLocked && showSelected,
                },
            )}
        >
            <div className="flex w-full items-center gap-x-3">
                <div>
                    <Avatar isLocked={isLocked} />
                </div>
                <div className="flex w-full min-w-0 flex-col items-start py-xs">
                    <div className="flex w-full min-w-0 items-center space-x-2">
                        <p className="account-title-color min-w-0 max-w-full truncate font-inter text-title-md">
                            {title}
                        </p>
                        {badgeText && badgeType ? (
                            badgeTooltipText ? (
                                <Tooltip text={badgeTooltipText} position={badgeTooltipPosition}>
                                    <Badge type={badgeType} label={badgeText} />
                                </Tooltip>
                            ) : (
                                <Badge type={badgeType} label={badgeText} />
                            )
                        ) : null}
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

                <div className="ml-auto flex items-center space-x-2 [&_button]:h-5 [&_button]:w-5 [&_svg]:h-5 [&_svg]:w-5">
                    <div className="account-icon-color flex items-center space-x-2 [&_button:not(.locked)]:invisible group-hover:[&_button:not(.locked)]:visible">
                        {onOptionsClick && (
                            <ButtonUnstyled onClick={onOptionsClick}>
                                <MoreHoriz />
                            </ButtonUnstyled>
                        )}
                        {onLockAccountClick &&
                            onUnlockAccountClick &&
                            (isLocked ? (
                                <div className="flex items-center">
                                    <ButtonUnstyled
                                        onClick={onUnlockAccountClick}
                                        testId="account-unlock"
                                        className="locked"
                                    >
                                        <LockLocked />
                                    </ButtonUnstyled>
                                </div>
                            ) : (
                                <ButtonUnstyled onClick={onLockAccountClick} testId="account-lock">
                                    <LockUnlocked />
                                </ButtonUnstyled>
                            ))}
                    </div>
                    {showSelected && (
                        <ButtonUnstyled>
                            {isSelected ? (
                                <CheckmarkFilled className="account-check-active-color h-5 w-5" />
                            ) : (
                                <RadioOff className="account-check-inactive-color h-5 w-5" />
                            )}
                        </ButtonUnstyled>
                    )}
                </div>
            </div>
        </div>
    );
}
