// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { useState } from 'react';
import cx from 'classnames';
import { AccountType } from './account.enums';
import { BACKGROUND_BADGE_COLORS, TEXT_COLORS, BADGE_TEXT_CLASS } from './account.classes';
import { Button, ButtonSize, ButtonType } from '../button';

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
    onThreeDotsClick: () => void;
    /**
     * Handler for the lock account icon click.
     */
    onLockAccount: () => void;
}

export function Account({
    title,
    subtitle,
    accountType,
    isLocked,
    onThreeDotsClick,
    onLockAccount,
}: AccountProps): React.JSX.Element {
    const [isHovered, setIsHovered] = useState<boolean>(false);

    const backgroundBadgeClasses = accountType ? BACKGROUND_BADGE_COLORS[accountType] : '';
    const textClasses = accountType ? TEXT_COLORS[accountType] : '';
    const circleFillClass = isLocked ? 'fill-neutral-80 dark:fill-neutral-30' : 'fill-primary-30';

    return (
        <div
            onMouseEnter={() => setIsHovered(true)}
            onMouseLeave={() => setIsHovered(false)}
            className="state-layer relative flex w-full items-center justify-between space-x-3 rounded-xl px-sm py-xs hover:cursor-pointer"
        >
            <div className="flex items-center space-x-3">
                <svg
                    xmlns="http://www.w3.org/2000/svg"
                    width="33"
                    height="32"
                    viewBox="0 0 33 32"
                    fill="none"
                >
                    <circle cx="16.5" cy="16" r="16" className={cx(circleFillClass)} />
                </svg>
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
                    <p className="text-body-sm text-neutral-40 dark:text-neutral-60">{subtitle}</p>{' '}
                </div>
            </div>
            <div
                className={cx('z-10 ml-auto flex items-center space-x-2', {
                    hidden: !isHovered && !isLocked,
                })}
            >
                <Button
                    size={ButtonSize.Small}
                    type={ButtonType.Ghost}
                    onClick={onThreeDotsClick}
                    icon={
                        <svg
                            xmlns="http://www.w3.org/2000/svg"
                            width="21"
                            height="20"
                            viewBox="0 0 21 20"
                            fill="none"
                            className="text-neutral-40 dark:text-neutral-60"
                        >
                            <path
                                d="M4.66667 11.6667C5.58714 11.6667 6.33333 10.9205 6.33333 10C6.33333 9.07957 5.58714 8.33337 4.66667 8.33337C3.74619 8.33337 3 9.07957 3 10C3 10.9205 3.74619 11.6667 4.66667 11.6667Z"
                                fill="currentColor"
                            />
                            <path
                                d="M12.1667 10C12.1667 10.9205 11.4205 11.6667 10.5 11.6667C9.57953 11.6667 8.83333 10.9205 8.83333 10C8.83333 9.07957 9.57953 8.33337 10.5 8.33337C11.4205 8.33337 12.1667 9.07957 12.1667 10Z"
                                fill="currentColor"
                            />
                            <path
                                d="M18 10C18 10.9205 17.2538 11.6667 16.3333 11.6667C15.4129 11.6667 14.6667 10.9205 14.6667 10C14.6667 9.07957 15.4129 8.33337 16.3333 8.33337C17.2538 8.33337 18 9.07957 18 10Z"
                                fill="currentColor"
                            />
                        </svg>
                    }
                />
                {isLocked ? (
                    <Button
                        size={ButtonSize.Small}
                        type={ButtonType.Ghost}
                        onClick={onLockAccount}
                        icon={
                            <svg
                                xmlns="http://www.w3.org/2000/svg"
                                width="21"
                                height="20"
                                viewBox="0 0 21 20"
                                fill="none"
                                className="text-neutral-40 dark:text-neutral-60"
                            >
                                <path
                                    fillRule="evenodd"
                                    clipRule="evenodd"
                                    d="M7.16683 9.16667V6.75373C7.16683 5.14634 8.48088 4.16667 10.0835 4.16667C11.6861 4.16667 12.9853 5.14634 12.9853 6.75373L13.0002 9.16667H14.6817L14.6668 6.75373C14.6668 4.40446 12.4258 2.5 10.0835 2.5C7.74122 2.5 5.50016 4.40446 5.50016 6.75373L5.50016 9.16667C4.51394 9.16667 3.8335 9.96854 3.8335 10.9577V15.709C3.8335 16.6981 4.63299 17.5 5.61921 17.5H14.5478C15.534 17.5 16.3335 16.6981 16.3335 15.709V10.9577C16.3335 9.96854 15.6531 9.16667 14.6668 9.16667H7.16683ZM11.6014 13.5597C11.6014 14.351 10.9618 14.9925 10.1728 14.9925C9.38381 14.9925 8.74422 14.351 8.74422 13.5597C8.74422 12.7684 9.38381 12.1269 10.1728 12.1269C10.9618 12.1269 11.6014 12.7684 11.6014 13.5597Z"
                                    fill="currentColor"
                                />
                            </svg>
                        }
                    />
                ) : (
                    <Button
                        size={ButtonSize.Small}
                        type={ButtonType.Ghost}
                        onClick={onLockAccount}
                        icon={
                            <svg
                                className={cx('text-neutral-40 dark:text-neutral-60', {
                                    hidden: !isHovered,
                                })}
                                xmlns="http://www.w3.org/2000/svg"
                                width="21"
                                height="20"
                                viewBox="0 0 21 20"
                                fill="none"
                            >
                                <path
                                    fillRule="evenodd"
                                    clipRule="evenodd"
                                    d="M7.16683 9.16667V6.75373C7.16683 5.41667 8.48088 4.16667 10.0835 4.16667C11.6861 4.16667 13.0002 5.41667 13.0002 6.66667H14.6668C14.6668 4.3174 12.4258 2.5 10.0835 2.5C7.74122 2.5 5.50016 4.40446 5.50016 6.75373L5.50016 9.16667C4.51394 9.16667 3.8335 9.96854 3.8335 10.9577V15.709C3.8335 16.6981 4.63299 17.5 5.61921 17.5H14.5478C15.534 17.5 16.3335 16.6981 16.3335 15.709V10.9577C16.3335 9.96854 15.534 9.16667 14.5478 9.16667H7.16683ZM11.6014 13.5597C11.6014 14.351 10.9618 14.9925 10.1728 14.9925C9.38381 14.9925 8.74422 14.351 8.74422 13.5597C8.74422 12.7684 9.38381 12.1269 10.1728 12.1269C10.9618 12.1269 11.6014 12.7684 11.6014 13.5597Z"
                                    fill="currentColor"
                                />
                            </svg>
                        }
                    />
                )}
            </div>
        </div>
    );
}
