// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import cx from 'classnames';

interface AddressProps {
    /**
     * The text of the address.
     */
    text: string;
    /**
     * Has copy icon (optional).
     */
    isCopyable?: boolean;
    /**
     * Has open icon  (optional).
     */
    isExternal?: boolean;
    /**
     * The onCopy event of the Address  (optional).
     */
    onCopy?: (e: React.MouseEvent<HTMLButtonElement>) => void;
    /**
     * The onOpen event of the Address  (optional).
     */
    onOpen?: (e: React.MouseEvent<HTMLButtonElement>) => void;
}

export function Address({
    text,
    isCopyable,
    isExternal,
    onCopy,
    onOpen,
}: AddressProps): React.JSX.Element {
    return (
        <div className="flex flex-row items-center justify-center gap-1 text-neutral-40 dark:text-neutral-60">
            <span className={cx('font-inter text-body-sm')}>{text}</span>
            {isCopyable && (
                <span onClick={onCopy}>
                    <svg
                        xmlns="http://www.w3.org/2000/svg"
                        width="16"
                        height="16"
                        viewBox="0 0 16 16"
                        fill="none"
                    >
                        <path
                            fill-rule="evenodd"
                            clip-rule="evenodd"
                            d="M2 4C2 2.89543 2.89543 2 4 2H9.33333C10.4379 2 11.3333 2.89543 11.3333 4V4.66667H12C13.1046 4.66667 14 5.5621 14 6.66667V12C14 13.1046 13.1046 14 12 14H6.66667C5.5621 14 4.66667 13.1046 4.66667 12V11.3333H4C2.89543 11.3333 2 10.4379 2 9.33333V4ZM6 12C6 12.3682 6.29848 12.6667 6.66667 12.6667H12C12.3682 12.6667 12.6667 12.3682 12.6667 12V6.66667C12.6667 6.29848 12.3682 6 12 6H6.66667C6.29848 6 6 6.29848 6 6.66667V12ZM10 4.66667H6.66667C5.5621 4.66667 4.66667 5.5621 4.66667 6.66667V10H4C3.63181 10 3.33333 9.70152 3.33333 9.33333V4C3.33333 3.63181 3.63181 3.33333 4 3.33333H9.33333C9.70152 3.33333 10 3.63181 10 4V4.66667Z"
                            fill="currentColor"
                        />
                    </svg>
                </span>
            )}
            {isExternal && (
                <span onClick={onOpen}>
                    <svg
                        xmlns="http://www.w3.org/2000/svg"
                        width="16"
                        height="16"
                        viewBox="0 0 16 16"
                        fill="none"
                    >
                        <path
                            d="M11.655 4.19343C11.5915 4.13042 11.5185 4.08281 11.4406 4.05059C11.362 4.01799 11.2758 4 11.1854 4L5.80474 4C5.43655 4 5.13807 4.29848 5.13807 4.66667C5.13807 5.03486 5.43655 5.33333 5.80474 5.33333L9.57597 5.33333L4.19526 10.714C3.93491 10.9744 3.93491 11.3965 4.19526 11.6569C4.45561 11.9172 4.87772 11.9172 5.13807 11.6569L10.5188 6.27614V10.0474C10.5188 10.4156 10.8173 10.714 11.1854 10.714C11.5536 10.714 11.8521 10.4156 11.8521 10.0474L11.8521 4.6672C11.8521 4.57639 11.834 4.48927 11.8011 4.41037C11.7687 4.33263 11.7211 4.25977 11.658 4.19642L11.6569 4.19526L11.655 4.19343Z"
                            fill="currentColor"
                        />
                    </svg>
                </span>
            )}
        </div>
    );
}
