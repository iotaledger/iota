// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import cx from 'classnames';
import { Button, ButtonSize, ButtonType } from '../button';

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
    const titleCenteredClasses = titleCentered ? 'text-center' : hasLeftIcon ? 'ml-1' : '';
    const keepSpaceForIcon = titleCentered && (!hasLeftIcon || !hasRightIcon);

    return (
        <div className="flex min-h-[56px] w-full items-center bg-neutral-100 px-lg pb-xs pt-sm text-neutral-10 dark:bg-neutral-6 dark:text-neutral-92">
            {hasLeftIcon ? (
                <Button
                    size={ButtonSize.Small}
                    type={ButtonType.Ghost}
                    onClick={onBack}
                    icon={
                        <svg
                            xmlns="http://www.w3.org/2000/svg"
                            width="20"
                            height="20"
                            viewBox="0 0 20 20"
                            fill="none"
                        >
                            <path
                                d="M3.33301 10.0032C3.33344 10.115 3.35589 10.2216 3.39624 10.3189C3.43691 10.4172 3.49719 10.5093 3.57709 10.5892L8.57709 15.5892C8.90252 15.9147 9.43016 15.9147 9.7556 15.5892C10.081 15.2638 10.081 14.7361 9.7556 14.4107L6.17819 10.8333H15.833C16.2932 10.8333 16.6663 10.4602 16.6663 9.99996C16.6663 9.53972 16.2932 9.16663 15.833 9.16663H6.17819L9.7556 5.58922C10.081 5.26378 10.081 4.73614 9.7556 4.4107C9.43016 4.08527 8.90252 4.08527 8.57709 4.4107L3.57756 9.41023C3.49729 9.4905 3.43634 9.58355 3.39567 9.68236C3.35553 9.77964 3.33328 9.88619 3.33301 9.99791L3.33301 9.99996L3.33301 10.0032Z"
                                fill="currentColor"
                            />
                        </svg>
                    }
                />
            ) : (
                keepSpaceForIcon && <div className="w-9" />
            )}

            <div className={cx('flex-grow', titleCenteredClasses)}>
                <span className="font-inter text-title-lg">{title}</span>
            </div>

            {hasRightIcon ? (
                <Button
                    size={ButtonSize.Small}
                    type={ButtonType.Ghost}
                    onClick={onClose}
                    icon={
                        <svg
                            xmlns="http://www.w3.org/2000/svg"
                            width="20"
                            height="20"
                            viewBox="0 0 20 20"
                            fill="none"
                        >
                            <path
                                fill-rule="evenodd"
                                clip-rule="evenodd"
                                d="M15.5896 5.58934C15.915 5.2639 15.915 4.73626 15.5896 4.41083C15.2641 4.08539 14.7365 4.08539 14.4111 4.41083L10.0002 8.82167L5.58978 4.41123C5.26435 4.0858 4.73671 4.0858 4.41127 4.41123C4.08584 4.73667 4.08584 5.26431 4.41127 5.58974L8.82171 10.0002L4.41107 14.4108C4.08563 14.7363 4.08563 15.2639 4.41107 15.5893C4.73651 15.9148 5.26414 15.9148 5.58958 15.5893L10.0002 11.1787L14.4113 15.5897C14.7367 15.9152 15.2643 15.9152 15.5898 15.5897C15.9152 15.2643 15.9152 14.7367 15.5898 14.4112L11.1787 10.0002L15.5896 5.58934Z"
                                fill="currentColor"
                            />
                        </svg>
                    }
                />
            ) : (
                keepSpaceForIcon && <div className="w-9" />
            )}
        </div>
    );
}
